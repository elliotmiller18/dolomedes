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

    // TODO: we should somehow discourage rejoining on the same node id to prevent different nodes
    // from having conflicting versions of home pages
    pub fn join_network(&mut self, genesis_nodes: Vec<NodeContact>) -> Result<()> {
        self.routing_table.insert_nodes_without_ping(genesis_nodes);
        ensure!(!self.routing_table.is_empty());

        let pow_nonce: U256 =
            crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);
        let join_message = Message::new(
            MessageType::JoinNetwork(
                self.port,
                pow_nonce.clone(),
                self.signing_key.verifying_key(),
            ),
            self.signing_key.clone(),
        );

        //TODO: this is pretty ugly (especially to do twice), will fix when we get better structure with mutexes on buckets.
        // or maybe just reshape this? or just not returning a reference yk idrk...
        let nodes: Vec<NodeContact> = self.routing_table.nodes().map(|c| c.clone()).collect();
        for node in nodes {
            self.send(join_message.clone(), &node)?;
        }

        let store_nodes = self.routing_table.store(self.node_id, pow_nonce.to_le_bytes().as_slice(), true)?;

        for node in store_nodes {
            self.send(join_message.clone(), &node)?;
        }

        Ok(())
    }

    pub async fn find_owner(file: FileId) -> Option<NodeContact> {
        unimplemented!();
    }

    // just a note for future implementation, the smartest design is probably one where a node can request chunks of arbitrary
    // size from owners and they can set their own rate limits rather than requesting full files.
    pub async fn request_file(owners: Vec<&NodeContact>, file: FileId) -> Option<TcpStream> {
        unimplemented!()
    }

    //TODO: I'm concerned that nodes will converge on similar k-buckets for a file and if it's popular, we could have an
    // extremely popular file effectively capped at 8 seeders -- find a way to fix this
    // (maybe if we're unable to handle a request we can return a node that the requester is unlikely to have (eg our newest node?)
    pub async fn handle_file_request(file: FileId) -> Result<()> {
        unimplemented!()
    }

    fn send(&mut self, message: Message, recipient: &NodeContact) -> Result<()> {
        // note that here we should adjust our table based on who fails to respond. if
        // something times out that means that node needs to be evicted.

        // also im thinking when this errors it's like an OS or Network error, not just
        // that we couldn't find the sender. maybe return Result<bool>?
        todo!();
    }

    fn verify(mut self, message: Message, verifying_key: VerifyingKey) -> bool {
        todo!();
    }
}

// this is needed as a type param for client so it's not in client
pub async fn ping(contact: &NodeContact) -> bool {
    unimplemented!()
}
