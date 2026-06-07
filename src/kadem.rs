use anyhow::{Result, ensure};
use crypto_bigint::U256;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::Mutex;

pub type NodeId = U256;
/// This is the variable "K" referred to in K-Buckets and all over the Kademlia paper
const BUCKET_SIZE: usize = 8;

#[derive(Clone, PartialEq, Eq)]
pub struct NodeContact {
    //UDP port
    pub port: u16,
    pub node_id: NodeId,
    pub ip: std::net::IpAddr,
}

pub struct Kademlia {
    // index zero has a completey different prefix,
    // index one has one matching bit,
    // index two has two, all the way to 256 (which is us)
    routing_table: Vec<Mutex<VecDeque<NodeContact>>>,
    node_id: NodeId,
}
impl Kademlia {
    pub const BUCKET_SIZE: usize = BUCKET_SIZE;

    pub fn new(node_id: NodeId) -> Self {
        Self {
            //OPTIMIZATION: add a floor to this that tells us what the first element of the routing table
            // with contacts in it is. chances are we're not gonna fill 0-200 in testing and even if
            // this grew to ipfs scale we'd still never fill most of them,
            // or even better just use a trie (although in this case a b-tree is a trie)
            routing_table: (0..256)
                .map(|_| Mutex::new(VecDeque::with_capacity(BUCKET_SIZE)))
                .collect(),
            node_id,
        }
    }

    pub fn k_closest(&self, node_id: NodeId) -> Result<Vec<NodeContact>> {
        ensure!(node_id != self.node_id, "trying to find ourself");
        Ok(self.closest_known_contacts(node_id))
    }

    pub async fn try_insert<F, Fut>(&self, contact: &NodeContact, ping: F) -> bool
    where
        F: FnOnce(NodeContact) -> Fut,
        Fut: Future<Output = bool>,
    {
        let bucket = self.bucket_for(contact.node_id);
        Self::update_bucket(bucket, contact, ping).await
    }

    pub async fn update_bucket<F, Fut>(
        bucket: &Mutex<VecDeque<NodeContact>>,
        contact: &NodeContact,
        ping: F,
    ) -> bool
    where
        F: FnOnce(NodeContact) -> Fut,
        Fut: Future<Output = bool>,
    {
        // evict_candidate is only set when the bucket is full and we need to ping the oldest node.
        // the lock is dropped before the ping await so this future stays Send.
        let evict_candidate = {
            let mut guard = bucket.lock().unwrap();
            if let Some(pos) = guard
                .iter()
                .position(|known_contact| known_contact.node_id == contact.node_id)
            {
                // this implicitly allows for us to easily update ip addresses and ports in case of a quick reconfig,
                // allows for nice graceful disconnect/reconnect cause sometimes someone wants to turn on a vpn or
                // whatever
                guard.remove(pos).unwrap();
                guard.push_front(contact.clone());
                return true;
            } else if guard.len() < Kademlia::BUCKET_SIZE {
                guard.push_front(contact.clone());
                return true;
            } else {
                guard.pop_back().unwrap()
            }
        };

        if ping(evict_candidate.clone()).await {
            bucket.lock().unwrap().push_front(evict_candidate);
            false
        } else {
            bucket.lock().unwrap().push_front(contact.clone());
            true
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

    pub fn bucket_for(&self, node_id: NodeId) -> &Mutex<VecDeque<NodeContact>> {
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

    pub fn xor_distance(a: NodeId, b: NodeId) -> NodeId {
        a ^ b
    }
}
