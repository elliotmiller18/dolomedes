//TODO: this file blows and is full of issues, rewrite. also maybe rename from client? idk
// cause we implement client mostly in proto.rs

pub mod cli;
mod messages;
pub mod request;
pub mod response;

use crate::kadem::{Kademlia, NodeContact, NodeId};
use ed25519_dalek::SigningKey;
use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 31460;
pub const DEFAULT_CONFIG_PATH: &str = "dolomedes.cfg";
pub const DEFAULT_DATA_DIR: &str = "dolomedes/data";

pub struct DolomedesClient<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    pub port: u16,
    pub datadir: PathBuf,
    pub signing_key: SigningKey,
    pub node_id: NodeId,
    pub routing_table: Kademlia<F>,
    //TODO: should probably have some ds with contact -> connection pool here
}
