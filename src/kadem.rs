use anyhow::{Result, ensure};
use crypto_bigint::U256;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::Mutex;

pub type NodeId = U256;
pub type KBucket = Mutex<VecDeque<NodeContact>>;
/// This is the variable "K" referred to in K-Buckets and all over the Kademlia paper
const BUCKET_SIZE: usize = 8;

#[derive(Clone, PartialEq)]
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

pub struct Kademlia {
    // index zero has a completey different prefix,
    // index one has one matching bit,
    // index two has two, all the way to 256 (which is us)
    routing_table: Vec<KBucket>,
    stores: HashMap<NodeId, Box<[u8]>>,
    node_id: NodeId,
}
impl Kademlia {
    pub const BUCKET_SIZE: usize = BUCKET_SIZE;

    pub fn new(node_id: NodeId) -> Self {
        Self {
            routing_table: (0..256)
                .map(|_| KBucket::new(VecDeque::with_capacity(BUCKET_SIZE)))
                .collect(),
            //OPTIMIZATION: add a floor to this that tells us what the first element of the routing table
            // with contacts in it is. chances are we're not gonna fill 0-200 in testing and even if
            // this grew to ipfs scale we'd still never fill most of them,
            // or even better just use a trie (although in this case a b-tree is a trie)
            stores: HashMap::new(),
            node_id,
        }
    }

    pub fn find_node(&self, node_id: NodeId) -> Result<Vec<NodeContact>> {
        ensure!(node_id != self.node_id, "trying to find ourself");
        Ok(self.closest_known_contacts(node_id))
    }

    pub fn find_value(&self, key: NodeId) -> Result<FindValueResult> {
        match self.stores.get(&key) {
            Some(path) => Ok(FindValueResult::Data(path.to_owned())),
            None => Ok(FindValueResult::Contact(self.find_node(key)?)),
        }
    }

    /// update the routing table when we communicate with a
    /// node, confirming that it's alive

    //TODO: makes it so that this doesn't lock up the whole routing table using nice mutexes.
    pub async fn update_bucket<P, Fut>(bucket: &mut KBucket, contact: NodeContact, ping: P)
    where
        P: FnOnce(&NodeContact) -> Fut,
        Fut: Future<Output = bool>,
    {
        let mut bucket = bucket.lock().unwrap();

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
            let evicted = bucket.pop_back().unwrap();
            if ping(&evicted).await {
                bucket.push_front(evicted);
            } else {
                bucket.push_front(contact);
            }
        }
        assert!(bucket.len() <= Self::BUCKET_SIZE);
    }

    /// Response to a STORE rpc.
    /// Returns closer Nodes that the store should be forwarded to.
    /// If there are less than K closer nodes or force_save is set we will also save the store.
    pub fn store<R: std::io::Read>(
        &mut self,
        key: NodeId,
        mut reader: R,
        force_save: bool,
    ) -> Result<Vec<NodeContact>> {
        let self_distance = Self::xor_distance(key, self.node_id);
        let closer_contacts: Vec<NodeContact> = self
            .closest_known_contacts(key)
            .into_iter()
            .filter(|contact| Self::xor_distance(contact.node_id, key) < self_distance)
            .take(Self::BUCKET_SIZE)
            .collect();

        let should_save = closer_contacts.len() < Self::BUCKET_SIZE
            || closer_contacts
                .iter()
                .any(|c| Self::xor_distance(key, c.node_id) > self_distance);

        if should_save || force_save {
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;
            self.stores.insert(key, buffer.into_boxed_slice());
        }

        Ok(closer_contacts)
    }

    pub fn evict_node(&mut self, victim: NodeId) {
        todo!("remove node from routing table")
    }

    // assumes that all inserted nodes have been recently confirmed to be live and skips ping
    pub fn try_insert_node_without_ping(&mut self, node: NodeContact) {
        let i = self.routing_index(node.node_id);
        let mut bucket = self.routing_table[i].lock().unwrap();
        // if there's space, insert, otherwise skip because we don't evict older nodes
        // in favor of newer nodes unless they fail to respond to a ping and the whole
        // point of this fn is that we AREN'T pinging
        if bucket.len() != BUCKET_SIZE {
            assert!(bucket.len() < BUCKET_SIZE);
            bucket.push_front(node);
        }
    }

    pub fn len(&self) -> usize {
        self.routing_table
            .iter()
            .map(|bucket| bucket.lock().unwrap().len())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bucket_for(&self, node_id: NodeId) -> &KBucket {
        &self.routing_table[self.routing_index(node_id)]
    }

    /// returns the number of matching leading bits of a node id and our node id
    fn routing_index(&self, id: NodeId) -> usize {
        assert!(
            id != self.node_id,
            "trying to find routing index of ourselves"
        );
        Self::xor_distance(id, self.node_id)
            .leading_zeros()
            .try_into()
            .unwrap()
    }

    //TODO: now that we have mutexes this is a bit gross, no?
    /// returns the k closest known contacts to target, if routing table has under k nodes it returns all nodes in the routing table
    fn closest_known_contacts(&self, target: NodeId) -> Vec<NodeContact> {
        // note: in this function (and elsewhere in this file) further/closer refer to ~~xor distance~~ which is described in the kademlia paper
        // all xor distance is is interpreting the size of a ^ b as the distance from a -> b.
        let routing_index = if target == self.node_id {
            self.routing_table.len()
        } else {
            self.routing_index(target)
        };

        // closer because nodes with an index >= routing index will always have a lower xor distance
        // than nodes that have an index < routing index, see kademlia paper or just read routing_index()
        // it's intuitive
        let mut contacts: Vec<NodeContact> = self.routing_table
            [routing_index..self.routing_table.len()]
            .iter()
            .flat_map(|bucket| bucket.lock().unwrap().iter().cloned().collect::<Vec<_>>())
            .take(Self::BUCKET_SIZE)
            .collect();

        if contacts.len() < Self::BUCKET_SIZE {
            contacts.extend(
                // here we take 16 because it guarantees that if we simply sort by xor distance later
                // we will get exactly the remaining closest nodes we know about, i believe any less
                // and we could get a suboptimal one
                self.routing_table[0..routing_index]
                    .iter()
                    .rev()
                    .flat_map(|bucket| bucket.lock().unwrap().iter().cloned().collect::<Vec<_>>())
                    .take(Self::BUCKET_SIZE * 2),
            );
        }

        contacts.sort_unstable_by(|a, b| {
            let dist_a = Self::xor_distance(a.node_id, target);
            let dist_b = Self::xor_distance(b.node_id, target);
            dist_a.cmp(&dist_b)
        });
        contacts.truncate(Self::BUCKET_SIZE);

        contacts
    }

    fn xor_distance(a: NodeId, b: NodeId) -> NodeId {
        a ^ b
    }
}
