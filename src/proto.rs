use std::net::{SocketAddr, TcpStream};

use anyhow::Result;

use crate::client::DolomedesClient;
use crate::kadem::{Kademlia, NodeContact};

pub type FileId = [u8; 32];

impl<F> DolomedesClient<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    pub fn join_network(genesis_nodes: Vec<NodeContact>) -> Result<()> {
        unimplemented!()
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
    //extremely popular file effectively capped at 8 seeders -- find a way to fix this 
    // (maybe if we're unable to handle a request we can return a node that the requester is unlikely to have (eg our newest node?)
    pub async fn handle_file_request(file: FileId) -> Result<()> {
        unimplemented!()
    }
}

// this is needed as a type param for client so it's not in client
pub async fn ping(contact: &NodeContact) -> bool {
    unimplemented!()
}
