use crate::kadem::NodeContact;
/// This file is handles the all requests the DolomedesClients can issue
use crate::{
    client::{
        DolomedesClient,
        messages::{Message, MessageBody},
    },
    kadem::{Kademlia, NodeId},
};
use std::{cmp::Ordering, collections::HashSet};

use anyhow::{Result, bail, ensure};
use binary_heap_plus::BinaryHeap;
use crypto_bigint::U256;

pub type FileId = U256;
pub const POW_LEADING_ZEROES: usize = 24;

fn order_nodes_by_xor_distance(file: FileId, a: &NodeContact, b: &NodeContact) -> Ordering {
    Kademlia::xor_distance(file, b.node_id)
        .cmp(&Kademlia::xor_distance(file, a.node_id))
        .then_with(|| b.node_id.cmp(&a.node_id))
}

impl DolomedesClient {
    /// join the dolomedes network for the **first** time, or if your routing table is lost.
    pub async fn join_network(&self, genesis_nodes: Vec<NodeContact>) -> Result<()> {
        let pow_nonce: U256 =
            crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);
        let join_message = Message::new(
            MessageBody::JoinNetwork {
                port: self.port,
                nonce: pow_nonce,
                verifying_key: self.signing_key.verifying_key(),
            },
            self.node_id,
            &self.signing_key,
        );

        for node in genesis_nodes {
            match self.send(&join_message, &node).await?.body() {
                MessageBody::JoinAck => {}
                _ => {
                    tracing::warn!("genesis node {} failed to respond properly", node.node_id);
                }
            }
        }
        ensure!(!self.routing_table.is_empty());

        Ok(())
    }

    //TODO: return Result<bool> when we implement writing kademlia to disk, cause an Err shouldn't cause the node
    // to be kicked from the table, only an Ok(false) should.
    pub(crate) async fn ping(&self, contact: &NodeContact) -> bool {
        let message = Message::new(MessageBody::Ping, self.node_id, &self.signing_key);
        let response = self.send(&message, contact).await;
        response.is_ok_and(|message| !matches!(message.body(), MessageBody::PingAck))
    }

    pub async fn declare_seed(&self, file_id: FileId) -> Result<()> {
        let k_closest = self.routing_table.k_closest(file_id)?;
        let message = Message::new(
            MessageBody::DeclareSeed { file_id },
            self.node_id,
            &self.signing_key,
        );
        for node in k_closest {
            match self.send(&message, &node).await?.body() {
                MessageBody::SeedAck => {}
                _ => tracing::warn!(
                    "node {} failed to acknowledge seed declaration",
                    node.node_id
                ),
            }
        }
        Ok(())
    }

    pub async fn acknowledge_seed(&self, requester: &NodeContact, file_id: FileId) -> Result<()> {
        self.seeders
            .lock()
            .unwrap()
            .entry(file_id)
            .or_default()
            .push(requester.node_id);
        self.send_discriminant(MessageBody::SeedAck, requester)
            .await
    }

    pub async fn serve_nodes(&self, target: NodeId, requester: &NodeContact) -> Result<()> {
        let k_closest = self.routing_table.k_closest(target).unwrap();
        let response = Message::new(
            MessageBody::Nodes { nodes: k_closest },
            self.node_id,
            &self.signing_key,
        );
        self.fire(&response, requester).await
    }

    pub(crate) async fn find_seeders(
        &self,
        file: FileId,
        candidates: impl Iterator<Item = NodeContact>,
    ) -> Result<Vec<NodeContact>> {
        // priority q of nodes by xor distance from target. nodes know more nodes closer to themselves
        // so we want to keep querying the closest node we know to the file until one has it
        let mut seen: HashSet<NodeId> = HashSet::new();
        let mut nodes = BinaryHeap::new_by(|a: &NodeContact, b: &NodeContact| {
            order_nodes_by_xor_distance(file, a, b)
        });
        nodes.extend(candidates.filter(|contact| seen.insert(contact.node_id)));

        while let Some(node) = nodes.pop() {
            let ownership_check = Message::new(
                MessageBody::GetSeeders { file_id: file },
                self.node_id,
                &self.signing_key,
            );
            match self.send(&ownership_check, &node).await?.body() {
                MessageBody::Error { code } => {
                    ensure!(code == Self::ERR_DOESNT_OWN_FILE);
                }
                MessageBody::Nodes { nodes } => {
                    return Ok(nodes);
                }
                _ => {}
            }

            let find_owners = Message::new(
                MessageBody::GetOwners { file_id: file },
                self.node_id,
                &self.signing_key,
            );
            match self.send(&find_owners, &node).await?.body() {
                MessageBody::Nodes {
                    nodes: closer_nodes,
                } => {
                    for node in closer_nodes {
                        if seen.insert(node.node_id) {
                            nodes.push(node);
                        }
                    }
                }
                _ => {
                    continue;
                }
            }
        }

        bail!(
            "routing table sputtered out while looking for file {file}, did nukes drop? am i just dumb?"
        );
    }
}
