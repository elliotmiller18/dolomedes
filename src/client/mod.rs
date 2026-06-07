pub mod fileshare;
mod messages;
pub mod routing;
pub mod setup;

use crate::client::routing::FileId;
use crate::kadem::{Kademlia, NodeId};
use anyhow::Result;
use ed25519_dalek::SigningKey;
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Mutex;

pub const DEFAULT_PORT: u16 = 31460;
pub const DEFAULT_CONFIG_PATH: &str = "dolomedes.cfg";
pub const DEFAULT_DATA_DIR: &str = "dolomedes/data";

pub struct DolomedesClient {
    pub port: u16,
    pub datadir: PathBuf,
    pub signing_key: SigningKey,
    pub node_id: NodeId,
    pub routing_table: Kademlia,
    pub seeders: Mutex<HashMap<FileId, Vec<NodeId>>>,
}

impl DolomedesClient {
    pub async fn serve(&self) -> Result<Infallible> {
        todo!();
    }
}
