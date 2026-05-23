use std::path::PathBuf;

use crate::client::DolomedesClient;
use crate::client::messages::{Message, MessageType};
use crate::client::routing::FileId;
use crate::kadem::{Kademlia, NodeContact, NodeId};
use anyhow::{Result, bail};
use binary_heap_plus::BinaryHeap;
use std::cmp::Ordering;
use std::collections::HashSet;

fn order_nodes_by_xor_distance(file: FileId, a: &NodeContact, b: &NodeContact) -> Ordering {
    Kademlia::xor_distance(file, b.node_id)
        .cmp(&Kademlia::xor_distance(file, a.node_id))
        .then_with(|| b.node_id.cmp(&a.node_id))
}

impl DolomedesClient {
    //TODO: I'm concerned that nodes will converge on similar k-buckets for a file and if it's popular, we could have an
    // extremely popular file effectively capped at 8 seeders -- find a way to fix this
    // (maybe if we're unable to handle a request we can return a node that the requester is unlikely to have (eg our newest node?)
    pub async fn handle_chunk_request(file: FileId) -> Result<()> {
        todo!()
    }

    //TODO: should impement these functions so that they get a vec of mutexes around the k buckets that they should be
    // querying rather than needing a full mutable reference to the routing table, as we won't be able to have multiple threads up at once
    // all mutably borrowing the routing table

    // just a note for future implementation, the smartest design is probably one where a node can request chunks of arbitrary
    // size from owners and they can set their own rate limits rather than requesting full files.
    pub async fn download_file(&self, file: FileId, path: PathBuf) -> Result<()> {
        todo!()
    }

    async fn find_owners(
        &self,
        file: FileId,
        candidates: impl Iterator<Item = &NodeContact>,
    ) -> Result<Vec<NodeContact>> {
        // priority q of nodes by xor distance from target. nodes know more nodes closer to themselves
        // so we want to keep querying the closest node we know to the file until one has it
        let mut seen: HashSet<NodeId> = HashSet::new();
        let mut nodes = BinaryHeap::new_by(|a: &NodeContact, b: &NodeContact| {
            order_nodes_by_xor_distance(file, a, b)
        });
        nodes.extend(
            candidates
                .filter(|contact| seen.insert(contact.node_id))
                .cloned(),
        );

        while let Some(node) = nodes.pop() {
            let message = Message::new(
                MessageType::FindOwners { file_id: file },
                self.node_id,
                &self.signing_key,
            );
            let response = self.send(&message, &node).await?;

            match MessageType::from_payload(response.payload) {
                MessageType::Owners { owners } => {
                    let bucket = self.routing_table.bucket_for(node.node_id);
                    self.insert_with_ping(bucket, &node).await;
                    return Ok(owners);
                }
                MessageType::Nodes {
                    nodes: closer_nodes,
                } => {
                    let bucket = self.routing_table.bucket_for(node.node_id);
                    self.insert_with_ping(bucket, &node).await;
                    for node in closer_nodes {
                        if seen.insert(node.node_id) {
                            nodes.push(node);
                        }
                    }
                }
                _ => {
                    self.routing_table.evict(node.node_id).await;
                    continue;
                }
            }
        }

        bail!(
            "routing table sputtered out while looking for file {file}, did nukes drop? am i just dumb?"
        );
    }
}
