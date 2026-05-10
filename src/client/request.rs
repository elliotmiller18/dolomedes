/// This file is handles the all requests the DolomedesClients can issue
use crate::{client::{
    DolomedesClient,
    messages::{Message, MessageType},
}};
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
        let recipients = self.routing_table.store(key, value.as_ref(), true)?;
        let store_message = Message::new(
            MessageType::Store(32, self.node_id, value),
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

    //TODO: should impement these functions so that they get a vec of mutexes around the k buckets that they should be
    // querying rather than needing a full mutable reference to the routing table, as we won't be able to have multiple threads up at once
    // all mutably borrowing the routing table

    // just a note for future implementation, the smartest design is probably one where a node can request chunks of arbitrary
    // size from owners and they can set their own rate limits rather than requesting full files.
    pub async fn request_file(&mut self, file: FileId) -> Result<()> {
        todo!()
    }

    /// sends a message to NodeContact and returns whether or not it got a response.
    fn send(&self, message: &Message, recipient: &NodeContact) -> Result<Message> {
        //TODO: down the line MSG_ZEROCOPY might be useful for seeding, as we're sending the same or an almost identical packet
        // over and over to different sources.

        // note that here we should adjust our table based on who fails to respond. if
        // something times out that means that node needs to be evicted.

        // also im thinking when this errors it's like an OS or Network error, not just
        // that we couldn't find the sender. maybe return Result<bool>?
        todo!();
    }

    async fn ping(&mut self, receiver: &NodeContact) -> bool {
        let rpc_id = self.node_id;
        let message = Message::new(MessageType::Ping(rpc_id), &self.signing_key);

        match self.send(&message, receiver) {
            Ok(_) => true,
            Err(_) => false
        }
    }
}

