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
        todo!()
    }

    pub async fn find_owner() -> Option<NodeContact> {
        todo!()
    }

    pub async fn request_file(owner: &NodeContact, file: FileId) -> Option<TcpStream> {
        todo!()
    }

    pub async fn handle_file_request(file: FileId) -> Result<()> {
        todo!()
    }
}

// this is needed as a type param for client so it's not in client
pub async fn ping(contact: &NodeContact) -> bool {
    todo!()
}
