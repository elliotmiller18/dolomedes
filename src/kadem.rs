use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

pub type NodeId = [u8; 32];
/// This is the variable "K" referred to in K-Buckets and all over the Kademlia paper
const BUCKET_SIZE: usize = 8;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeContact {
    //UDP port
    pub port: u16,
    pub node_id: NodeId,
    pub ip: std::net::IpAddr,
}

pub enum FindValueResult {
    Contact(Vec<NodeContact>),
    Data(Box<[u8]>),
}

#[derive(Serialize, Deserialize)]
pub struct KademliaData {
    // index zero has a completey different prefix,
    // index one has one matching bit,
    // index two has two, all the way to 256 (which is us)
    
    //TODO: make the routing_table a vec of mutexes so that multiple async fns can use it
    // at the same time
    routing_table: Vec<VecDeque<NodeContact>>,
    stores: HashMap<NodeId, Box<[u8]>>,
    node_id: NodeId,
}

pub struct Kademlia<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    data: KademliaData,
    ping: F,
}

impl KademliaData {
    fn new(node_id: NodeId) -> Self {
        Self {
            routing_table: (0..256)
                .map(|_| VecDeque::with_capacity(BUCKET_SIZE))
                .collect(),
            //OPTIMIZATION: add a floor to this that tells us what the first element of the routing table
            // with contacts in it is. chances are we're not gonna fill 0-200 in testing and even if
            // this grew to ipfs scale we'd still never fill most of them,
            // or even better just use a trie (although in this case a b-tree is a trie)
            stores: HashMap::new(),
            node_id,
        }
    }
}

impl<F> Kademlia<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    pub const BUCKET_SIZE: usize = BUCKET_SIZE;

    pub fn new(node_id: NodeId, ping: F) -> Self {
        Self {
            data: KademliaData::new(node_id),
            ping,
        }
    }

    pub fn from_file(path: PathBuf, ping: F) -> Result<Self> {
        let file = std::fs::File::open(&path)
            .with_context(|| format!("failed to open routing table {}", path.display()))?;
        let reader = BufReader::new(file);

        let data = bincode::deserialize_from(reader).with_context(|| {
            format!(
                "failed to deserialize binary routing table {}",
                path.display()
            )
        })?;

        Ok(Self { data, ping })
    }

    pub fn to_file(&self, path: PathBuf) -> Result<()> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .with_context(|| format!("failed to open routing table {}", path.display()))?;
        let writer = BufWriter::new(file);

        bincode::serialize_into(writer, &self.data).with_context(|| {
            format!(
                "failed to serialize binary routing table {}",
                path.display()
            )
        })?;

        Ok(())
    }

    pub fn find_node(&self, node_id: NodeId) -> Result<Vec<NodeContact>> {
        ensure!(node_id != self.data.node_id, "trying to find ourself");
        Ok(self.closest_known_contacts(node_id))
    }

    pub fn find_value(&self, key: NodeId) -> Result<FindValueResult> {
        match self.data.stores.get(&key) {
            Some(path) => Ok(FindValueResult::Data(path.to_owned())),
            None => Ok(FindValueResult::Contact(self.find_node(key)?)),
        }
    }

    /// update the routing table when we communicate with a
    /// node, confirming that it's alive

    //TODO: makes it so that this doesn't lock up the whole routing table. we need a mutex on each bucket.
    pub async fn update_bucket(&mut self, contact: NodeContact) {
        let i = self.routing_index(contact.node_id);

        if self.data.routing_table.get(i).is_none() {
            // NOTE: this should never run but in case we optimize with lazily initing buckets later we will
            // have it for posterity sake
            let mut empty_bucket = VecDeque::new();
            empty_bucket.push_front(contact);
            self.data.routing_table.insert(i, empty_bucket);
            return;
        }

        let bucket = &mut self.data.routing_table[i];

        if bucket.len() < Self::BUCKET_SIZE {
            bucket.push_front(contact);
            return;
        }

        if let Some(pos) = bucket
            .iter()
            .position(|known_contact| known_contact.node_id == contact.node_id)
        {
            // this implicitly allows for us to easily update ip addresses and ports in case of a quick reconfig,
            // allows for nice graceful disconnect/reconnect cause sometimes someone wants to turn on a vpn or
            // whatever
            bucket.remove(pos).unwrap();
            bucket.push_front(contact);
            return;
        } else if bucket.len() < Self::BUCKET_SIZE {
            bucket.push_front(contact);
        } else {
            assert!(bucket.len() == Self::BUCKET_SIZE);
            let evicted = bucket.pop_back().unwrap();
            if (self.ping)(&evicted).await {
                bucket.push_front(evicted);
            } else {
                bucket.push_front(contact);
            }
        }
        assert!(bucket.len() <= Self::BUCKET_SIZE);
    }

    /// Returns closer Nodes that the store should be forwarded to.
    /// If there are less than K closer nodes or force_save is set we will also save the store.
    pub fn store<R: std::io::Read>(
        &mut self,
        key: NodeId,
        mut reader: R,
        force_save: bool,
    ) -> Result<Vec<NodeContact>> {
        let self_distance = Self::xor_distance(key, self.data.node_id);
        let closer_contacts: Vec<NodeContact> = self
            .closest_known_contacts(key)
            .into_iter()
            .filter(|contact| Self::xor_distance(contact.node_id, key) < self_distance)
            .take(Self::BUCKET_SIZE)
            .collect();

        if closer_contacts.len() <= Self::BUCKET_SIZE || force_save {
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;
            self.data.stores.insert(key, buffer.into_boxed_slice());
        }

        Ok(closer_contacts)
    }

    pub fn evict_node(&mut self, victim: NodeId) {
        unimplemented!("remove victim from routing table")
    }

    // meant for use with a replacement cache
    pub fn insert_nodes_without_ping(&mut self, nodes: Vec<NodeContact>) {
        unimplemented!("add buckets")
    }

    /// returns the number of matching leading bits of a node id and our node id
    fn routing_index(&self, id: NodeId) -> usize {
        assert!(
            id != self.data.node_id,
            "trying to find routing index of ourselves"
        );
        let mut i = 0;
        loop {
            if id[i] == self.data.node_id[i] {
                i += 1;
                continue;
            }
            let matching_high_bits: usize = (self.data.node_id[i] ^ id[i])
                .leading_zeros()
                .try_into()
                .unwrap();
            break (i * 8) + matching_high_bits;
        }
    }

    fn closest_known_contacts(&self, target: NodeId) -> Vec<NodeContact> {
        // note: in this function (and elsewhere in this file) further/closer refer to ~~xor distance~~ which is described in the kademlia paper
        // all xor distance is is interpreting the size of a ^ b as the distance from a -> b.
        let routing_index = if target == self.data.node_id {
            self.data.routing_table.len()
        } else {
            self.routing_index(target)
        };

        // closer because nodes with an index >= routing index will always have a lower xor distance
        // than nodes that have an index < routing index, see kademlia paper or just read routing_index()
        // it's intuitive
        let closer_buckets = &self.data.routing_table[routing_index..self.data.routing_table.len()];
        let mut contacts: Vec<&NodeContact> = closer_buckets
            .iter()
            .flatten()
            .take(Self::BUCKET_SIZE)
            .collect();

        if contacts.len() < Self::BUCKET_SIZE {
            let farther_buckets = &self.data.routing_table[0..routing_index];
            contacts.extend(
                // here we take 16 because it guarantees that if we simply sort by xor distance later
                // we will get exactly the remaining closest nodes we know about, i believe any less
                // and we could get a suboptimal one
                farther_buckets
                    .iter()
                    .rev()
                    .flatten()
                    .take(Self::BUCKET_SIZE * 2),
            );
        }

        contacts.sort_unstable_by(|a, b| {
            let dist_a = Self::xor_distance(a.node_id, target);
            let dist_b = Self::xor_distance(b.node_id, target);
            dist_a.cmp(&dist_b)
        });
        contacts.truncate(Self::BUCKET_SIZE);

        contacts.into_iter().cloned().collect()
    }

    fn xor_distance(a: NodeId, b: NodeId) -> NodeId {
        let mut result: NodeId = [0u8; 32];
        for i in 0..a.len() {
            result[i] = a[i] ^ b[i];
        }
        result
    }
}
