//TODO: this file blows and is full of issues, rewrite. also maybe rename from client? idk
// cause we implement client mostly in proto.rs

pub mod cli;
pub mod fileshare;
mod messages;
pub mod routing;

use crate::kadem::{Kademlia, NodeId};
use anyhow::Result;
use ed25519_dalek::SigningKey;
use std::{convert::Infallible, path::PathBuf};

pub const DEFAULT_PORT: u16 = 31460;
pub const DEFAULT_CONFIG_PATH: &str = "dolomedes.cfg";
pub const DEFAULT_DATA_DIR: &str = "dolomedes/data";

pub struct DolomedesClient {
    pub port: u16,
    pub datadir: PathBuf,
    pub signing_key: SigningKey,
    pub node_id: NodeId,
    pub routing_table: Kademlia,
}

impl DolomedesClient {
    pub fn serve(config_path: PathBuf) -> Result<Infallible> {
        let client = DolomedesClient::with_config(config_path)?;
        todo!();
    }
}
