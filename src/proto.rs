use std::net::TcpStream;
use std::path::PathBuf;

use anyhow::{Result, ensure};
use crypto_bigint::U256;
use ed25519_dalek::VerifyingKey;

use crate::client::DolomedesClient;
use crate::kadem::NodeContact;
use crate::messages::{Message, MessageType};

pub type FileId = U256;
pub const POW_LEADING_ZEROES: usize = 24;

impl<F> DolomedesClient<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    /// join the dolomedes network for the **first** time, or if your routing table is lost.
    pub fn join_network(&mut self, genesis_nodes: Vec<NodeContact>) -> Result<()> {
        let pow_nonce: U256 =
            crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);
        let join_message = Message::new(
            MessageType::JoinNetwork(
                self.port,
                pow_nonce,
                self.signing_key.verifying_key(),
            ),
            &self.signing_key,
        );

        // send join network to genesis nodes, 
        for node in genesis_nodes {
            if self.send(&join_message, &node)? {
                self.routing_table.try_insert_node_without_ping(node);   
            } else {
                tracing::warn!("genesis node {} failed to respond", node.node_id);
            }
        }
        ensure!(!self.routing_table.is_empty());

        let store_nodes =
            self.routing_table
                .store(self.node_id, pow_nonce.to_le_bytes().as_slice(), true)?;

        let store_message = Message::new(
            MessageType::Store(
                32,
                self.node_id,
                Box::from(pow_nonce.to_le_bytes().as_slice()),
            ),
            &self.signing_key,
        );

        for node in store_nodes {
            if !self.send(&store_message, &node)? {
                self.routing_table.evict_node(node.node_id);  
            }
        }

        Ok(())
    }

    // just a note for future implementation, the smartest design is probably one where a node can request chunks of arbitrary
    // size from owners and they can set their own rate limits rather than requesting full files.
    pub async fn request_file(&mut self, owners: Vec<&NodeContact>, file: FileId) -> Option<TcpStream> {
        todo!()
    }

    //TODO: I'm concerned that nodes will converge on similar k-buckets for a file and if it's popular, we could have an
    // extremely popular file effectively capped at 8 seeders -- find a way to fix this
    // (maybe if we're unable to handle a request we can return a node that the requester is unlikely to have (eg our newest node?)
    pub async fn handle_chunk_request(file: FileId) -> Result<()> {
        todo!()
    }

    /// sends a message to NodeContact and returns whether or not it got a response.
    fn send(&self, message: &Message, recipient: &NodeContact) -> Result<bool> {
        //TODO: down the line MSG_ZEROCOPY might be useful for seeding, as we're sending the same or an almost identical packet
        // over and over to different sources. 

        // note that here we should adjust our table based on who fails to respond. if
        // something times out that means that node needs to be evicted.

        // also im thinking when this errors it's like an OS or Network error, not just
        // that we couldn't find the sender. maybe return Result<bool>?
        todo!();
    }
}

// this is needed as a type param for client so it's not in client
pub async fn ping(contact: &NodeContact) -> bool {
    todo!()
}
