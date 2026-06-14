pub mod fileshare;
mod messages;
pub mod routing;
pub mod setup;

use crate::client::messages::{Message, MessageBody};
use crate::client::routing::FileId;
use crate::kadem::{Kademlia, NodeContact, NodeId};
use anyhow::Result;
use ed25519_dalek::SigningKey;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
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
    pub endpoint: quinn::Endpoint,
}

impl DolomedesClient {
    pub async fn serve(&self) -> Result<Infallible> {
        loop {
            let Some(incoming) = self.endpoint.accept().await else {
                anyhow::bail!("endpoint closed");
            };
            self.handle_incoming(incoming).await.unwrap();
        }
    }

    async fn handle_incoming(&self, incoming: quinn::Incoming) -> Result<()> {
        let conn = incoming.await?;
        let (mut tx, mut rx) = conn.accept_bi().await?;
        loop {
            match self.recv(&mut rx).await {
                //TODO: verify messages here
                Ok(msg) => self.request_to_handler(msg, &mut tx).await?,
                Err(_) => break,
            }
        }
        Ok(())
    }

    async fn request_to_handler(&self, msg: Message, tx: &mut quinn::SendStream) -> Result<()> {
        let sender_id = msg.node_id;
        match msg.body() {
            MessageBody::Ping => {
                self.send(
                    &Message::new(MessageBody::PingAck, self.node_id, &self.signing_key),
                    tx,
                )
                .await
            }
            MessageBody::GetNode { node } => self.serve_nodes(node, tx).await,
            MessageBody::GetOwners { file_id } => self.serve_nodes(file_id, tx).await,
            MessageBody::DeclareSeed { file_id } => {
                self.acknowledge_seed(sender_id, file_id, tx).await
            }
            MessageBody::GetSeeders { file_id } => self.serve_nodes(file_id, tx).await,
            MessageBody::JoinNetwork { .. } => todo!(),
            MessageBody::GetFileMetadata { file_id } => {
                let path = self.datadir.join(hex::encode(file_id.to_le_bytes()));
                self.serve_file_metadata(file_id, &path, tx).await
            }
            MessageBody::GetChunk {
                chunk_index,
                chunk_size,
                file_id,
            } => {
                let path = self.datadir.join(hex::encode(file_id.to_le_bytes()));
                self.serve_chunk(chunk_index, chunk_size, file_id, path, tx)
                    .await
            }
            _ => {
                self.send(
                    &Message::new(MessageBody::InvalidMessage, self.node_id, &self.signing_key),
                    tx,
                )
                .await
            }
        }
    }

    pub async fn open_connection(&self, node: &NodeContact) -> Result<quinn::Connection> {
        let addr = SocketAddr::new(node.ip, node.port);
        Ok(self.endpoint.connect(addr, "dolomedes")?.await?)
    }
}
