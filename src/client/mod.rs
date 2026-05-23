//TODO: this file blows and is full of issues, rewrite. also maybe rename from client? idk
// cause we implement client mostly in proto.rs

pub mod cli;
pub mod fileshare;
mod messages;
pub mod routing;

use crate::kadem::{Kademlia, NodeContact, NodeId};
use anyhow::Result;
use ed25519_dalek::SigningKey;
use std::{collections::VecDeque, convert::Infallible, path::PathBuf, sync::Mutex};

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

    /// update bucket given we've just recieved a nice response from contact
    pub async fn insert_with_ping(
        &self,
        bucket: &Mutex<VecDeque<NodeContact>>,
        contact: &NodeContact,
    ) {
        let mut bucket = bucket.lock().unwrap();

        if let Some(pos) = bucket
            .iter()
            .position(|known_contact| known_contact.node_id == contact.node_id)
        {
            // this implicitly allows for us to easily update ip addresses and ports in case of a quick reconfig,
            // allows for nice graceful disconnect/reconnect cause sometimes someone wants to turn on a vpn or
            // whatever
            bucket.remove(pos).unwrap();
            bucket.push_front(contact.clone());
            return;
        } else if bucket.len() < Kademlia::BUCKET_SIZE {
            bucket.push_front(contact.clone());
        } else {
            let evicted = bucket.pop_back().unwrap();
            if self.ping(&evicted).await {
                bucket.push_front(evicted);
            } else {
                bucket.push_front(contact.clone());
            }
        }
        assert!(bucket.len() <= Kademlia::BUCKET_SIZE);
    }
}
