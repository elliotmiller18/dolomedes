use std::net::{SocketAddr, TcpStream};

use anyhow::Result;

use crate::kadem::{Kademlia, NodeContact};

pub type FileId = [u8; 32];

//TODO: this whole file is probably easier if we wrap it in a proto or client struct

//TODO: this function will need genesis nodes
pub fn join_network(our_contact: &NodeContact) -> Result<TcpStream> {
    todo!()
}

pub async fn ping(contact: &NodeContact) -> bool {
    todo!()
}

pub async fn find_owner<F>(table: &Kademlia<F>) -> Option<NodeContact>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    todo!()
}

pub async fn request_file(owner: &NodeContact, file: FileId) -> Option<TcpStream> {
    todo!()
}

pub async fn handle_file_request(file: FileId) {

}
