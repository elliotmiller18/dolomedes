use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{BufReader, BufWriter, ErrorKind, Write};
use std::path::PathBuf;

pub type NodeId = [u8; 32];
const BUCKET_SIZE: usize = 8;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeContact {
    //UDP port
    port: u16,
    node_id: NodeId,
    ip: std::net::IpAddr,
}

pub enum FindValueResult {
    Contact(Vec<NodeContact>),
    File(PathBuf),
}

#[derive(Serialize, Deserialize)]
pub struct KademliaData {
    // index zero has a completey different prefix,
    // index one has one matching bit,
    // index two has two, all the way to 256 (which is us)
    routing_table: Vec<VecDeque<NodeContact>>,
    filepaths: HashMap<NodeId, PathBuf>,
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
            // this grew to ipfs scale we'd still never fill most of them
            filepaths: HashMap::new(),
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
        let data = serde_json::from_reader(reader)
            .with_context(|| format!("failed to deserialize routing table {}", path.display()))?;

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
        serde_json::to_writer(writer, &self.data)
            .with_context(|| format!("failed to serialize routing table {}", path.display()))?;

        Ok(())
    }

    pub fn find_node(&self, node_id: NodeId) -> Result<Vec<NodeContact>> {
        ensure!(node_id != self.data.node_id, "trying to find ourself");
        // note: in this function (and elsewhere in this file) further/closer refer to ~~xor distance~~ which is described in the kademlia paper
        // all xor distance is is interpreting the size of a ^ b as the distance from a -> b.
        let routing_index = self.routing_index(node_id);

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
            contacts.append(
                // here we take 16 because it guarantees that if we simply sort by xor distance later
                // we will get exactly the remaining closest nodes we know about, i believe any less
                // and we could get a suboptimal one
                &mut farther_buckets
                    .into_iter()
                    .rev()
                    .flatten()
                    .take(Self::BUCKET_SIZE * 2)
                    .collect(),
            );
        }

        contacts.sort_unstable_by(|a, b| {
            let dist_a = Self::xor_distance(a.node_id, node_id);
            let dist_b = Self::xor_distance(b.node_id, node_id);
            dist_a.cmp(&dist_b)
        });
        contacts.truncate(Self::BUCKET_SIZE);

        Ok(contacts.into_iter().cloned().collect())
    }

    pub fn find_value(&self, key: NodeId) -> Result<FindValueResult> {
        match self.data.filepaths.get(&key) {
            Some(path) => Ok(FindValueResult::File(path.to_owned())),
            None => Ok(FindValueResult::Contact(self.find_node(key)?)),
        }
    }

    /// update the routing table when we communicate with a
    /// node, confirming that it's alive

    //TODO: this shouldn't be async cause updating a bucket locks up the whole routing table
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

    // this is actually a good example of simple, idiomatic rust.
    // we're accepting any data structure that implements Read because this function only
    // needs to use .read() and it may introdue a burden on later development if we ever want to
    // write to this from anything that's not a BufReader.

    // this is especially important when you're writing a data structure

    //TODO: this is, however, a terrible example of efficient Rust. This is writing entire files 1 byte at a time
    // and is laughably slow, it is spec adherent but needs a refactor
    pub fn store<R: std::io::Read>(
        &mut self,
        key: NodeId,
        reader: R,
        destination: PathBuf,
    ) -> Result<()> {
        let mut file_writer = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&destination)
            .with_context(|| format!("failed to open {} for writing", destination.display()))?;

        for byte in reader.bytes() {
            match byte {
                Ok(byte) => {
                    file_writer
                        .write_all(&[byte])
                        .with_context(|| format!("failed to write to {}", destination.display()))?;
                }
                Err(e) => match e.kind() {
                    ErrorKind::Interrupted => continue,
                    ErrorKind::ConnectionReset | ErrorKind::UnexpectedEof => break,
                    // Something actually went wrong (e.g., disk full, permission denied)
                    _ => {
                        return Err(e).with_context(|| {
                            format!(
                                "failed while reading source bytes for {}",
                                destination.display()
                            )
                        });
                    }
                },
            }
        }

        file_writer
            .flush()
            .with_context(|| format!("failed to flush {}", destination.display()))?;

        self.data.filepaths.insert(key, destination);
        Ok(())
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

    fn xor_distance(a: NodeId, b: NodeId) -> NodeId {
        let mut result: NodeId = [0u8; 32];
        for i in 0..a.len() {
            result[i] = a[i] ^ b[i];
        }
        result
    }
}
