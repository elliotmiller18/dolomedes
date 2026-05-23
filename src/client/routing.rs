use crate::kadem::NodeContact;
/// This file is handles the all requests the DolomedesClients can issue
use crate::{
    client::{
        DolomedesClient,
        messages::{Message, MessageType},
    },
    kadem::Kademlia,
};
use std::{collections::VecDeque, sync::Mutex};

use anyhow::{Result, ensure};
use crypto_bigint::U256;

pub type FileId = U256;
pub const POW_LEADING_ZEROES: usize = 24;

impl DolomedesClient {
    /// join the dolomedes network for the **first** time, or if your routing table is lost.
    pub async fn join_network(&mut self, genesis_nodes: Vec<NodeContact>) -> Result<()> {
        let pow_nonce: U256 =
            crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);
        let join_message = Message::new(
            MessageType::JoinNetwork {
                port: self.port,
                nonce: pow_nonce,
                verifying_key: self.signing_key.verifying_key(),
            },
            self.node_id,
            &self.signing_key,
        );

        for node in genesis_nodes {
            match MessageType::from_payload(self.send(&join_message, &node).await?.payload) {
                MessageType::JoinAck => {
                    self.routing_table.insert(node);
                }
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
    pub async fn ping(&self, contact: &NodeContact) -> bool {
        let message = Message::new(MessageType::Ping, self.node_id, &self.signing_key);
        let response = self.send(&message, contact).await;
        response.is_ok_and(|message| {
            !matches!(
                MessageType::from_payload(message.payload),
                MessageType::PingAck
            )
        })
    }
}
