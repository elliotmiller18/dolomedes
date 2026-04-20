use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;

use anyhow::{Result, ensure};
use crypto_bigint::U256;
use ed25519_dalek::{SigningKey, VerifyingKey};

use crate::client::DolomedesClient;
use crate::kadem::{Kademlia, NodeContact};

pub type FileId = U256;
pub const POW_LEADING_ZEROES: usize = 24;

impl<F> DolomedesClient<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    fn send(signing_key: SigningKey, recipient: &NodeContact, payload: &[u8]) -> Result<()> {
        unimplemented!();
    }

    fn verify(verifying_key: VerifyingKey, payload: &[u8]) -> bool {
        unimplemented!();
    }
    //TODO: implement proper key verification in here right now this is entirely unsecured.

    /// join the dolomedes network for the **first** time, or if your routing table is lost.

    // TODO: we should somehow discourage rejoining on the same node id to prevent different nodes
    // from having conflicting versions of home pages
    pub fn join_network(
        &mut self,
        genesis_nodes: Vec<NodeContact>,
        home_file: Option<PathBuf>,
    ) -> Result<()> {
        self.routing_table.insert_nodes_without_ping(genesis_nodes);
        // store webpage or just store something arbitrary to join the network
        match home_file {
            Some(path) => {
                self.routing_table.store(
                    self.node_id,
                    std::fs::File::open_buffered(path)?,
                    true,
                )?;
            }
            None => {
                self.routing_table
                    .store(self.node_id, &[0xFFu8][..], true)?;
            }
        };

        let pow_nonce: U256 = crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);

        ensure!(!self.routing_table.is_empty());
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
}

// this is needed as a type param for client so it's not in client
pub async fn ping(contact: &NodeContact) -> bool {
    unimplemented!()
}
