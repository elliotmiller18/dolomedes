/// This file is handles the all requests the DolomedesClients can issue
use crate::client::{
    DolomedesClient,
    messages::{Message, MessageType},
};
use crate::kadem::NodeContact;

use anyhow::{Result, ensure};
use crypto_bigint::U256;

pub type FileId = U256;
pub const POW_LEADING_ZEROES: usize = 24;

impl DolomedesClient {
    /// join the dolomedes network for the **first** time, or if your routing table is lost.
    pub fn join_network(&mut self, genesis_nodes: Vec<NodeContact>) -> Result<()> {
        let pow_nonce: U256 =
            crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);
        let join_message = Message::new(
            MessageType::JoinNetwork(self.port, pow_nonce, self.signing_key.verifying_key()),
            self.node_id,
            &self.signing_key,
        );

        for node in genesis_nodes {
            match MessageType::from_payload(self.send(&join_message, &node)?.payload) {
                MessageType::JoinAck => {
                    self.routing_table.try_insert_node_without_ping(node);
                }
                _ => {
                    tracing::warn!("genesis node {} failed to respond properly", node.node_id);
                }
            }
        }
        ensure!(!self.routing_table.is_empty());

        self.store(self.node_id, Box::from(pow_nonce.to_le_bytes().as_slice()))?;

        Ok(())
    }

    pub fn store(&mut self, key: FileId, value: Box<[u8]>) -> Result<()> {
        //TODO: we need to also maintain seeder lists (lists of nodes actually seeding files)
        // here as the way it's supposed to work is that nodes near a value, so maybe we can add an
        // optional original sender field to the protocol
        let recipients = self.routing_table.store(key, value.as_ref(), true)?;
        let store_message = Message::new(
            MessageType::Store(32, self.node_id, value),
            self.node_id,
            &self.signing_key,
        );

        for node in recipients {
            if !matches!(
                MessageType::from_payload(self.send(&store_message, &node)?.payload),
                MessageType::StoreAck
            ) {
                self.routing_table.evict_node(node.node_id);
            }
        }

        Ok(())
    }

    pub fn ping(&mut self, contact: &NodeContact) -> bool {
        todo!()
    }
}
