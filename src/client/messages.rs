use crate::client::DolomedesClient;
use anyhow::Result;
use crypto_bigint::U256;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::client::routing::FileId;
use crate::kadem::{NodeContact, NodeId};

#[derive(Clone)]
pub struct Message {
    pub node_id: NodeId,
    pub payload: Box<[u8]>,
    signature: Signature,
    timestamp: u64,
}

impl Message {
    pub fn new(message_body: MessageBody, node_id: NodeId, signing_key: &SigningKey) -> Self {
        let payload = message_body.to_payload();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let to_sign = Self::signable_payload(&payload, timestamp);

        Self {
            node_id,
            payload,
            signature: signing_key.sign(&to_sign),
            timestamp,
        }
    }

    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        let to_verify = Self::signable_payload(&self.payload, self.timestamp);

        verifying_key
            .verify_strict(&to_verify, &self.signature)
            .is_ok()
    }

    fn signable_payload(payload: &[u8], timestamp: u64) -> Box<[u8]> {
        let mut buf = Vec::with_capacity(payload.len() + 8);
        buf.extend_from_slice(&payload);
        buf.extend_from_slice(&timestamp.to_le_bytes());
        buf.into_boxed_slice()
    }
}

// all file sizes in bytes
pub enum MessageBody {
    Ping,
    PingAck,
    Seed {
        file_id: FileId,
    },
    GetNode {
        node: NodeId,
    },
    GetOwners {
        file_id: NodeId,
    },
    Nodes {
        nodes: Vec<NodeContact>,
    },
    JoinNetwork {
        port: u16,
        nonce: U256,
        verifying_key: VerifyingKey,
    },
    JoinAck,
    GetFileMetadata {
        file_id: FileId,
    },
    FileMetadata {
        file_id: FileId,
        file_size: u64,
        file_name: String,
    },
    GetChunk {
        chunk_index: u32,
        chunk_size: u64,
        file_id: FileId,
    },
    Chunk {
        chunk_index: u32,
        chunk_size: u64,
        file_id: FileId,
        data: Box<[u8]>,
    },
    GetSeeders {
        file_id: FileId,
    },
    InvalidMessage,
    Error {
        code: i64,
    },
}

impl MessageBody {
    pub fn to_payload(self) -> Box<[u8]> {
        todo!()
    }

    pub fn from_payload(payload: Box<[u8]>) -> Self {
        todo!()
    }
}

impl DolomedesClient {
    //TODO: rename these two? send is supposed to be fire and forget but this shapes makes writing the protocol nicer
    /// sends a message to NodeContact and returns whether or not it got a response.
    pub(crate) async fn send(&self, message: &Message, recipient: &NodeContact) -> Result<Message> {
        //TODO: down the line MSG_ZEROCOPY might be useful for seeding, as we're sending the same or an almost identical packet
        // over and over to different sources.

        //TODO: centralize routing table updates here so handlers don't need to worry about semantics.
        // this means insertions as well as evictions.

        // also im thinking when this errors it's like an OS or Network error, not just
        // that we couldn't find the sender. maybe return Result<bool>?
        todo!();
    }

    pub(crate) async fn send_ack(
        &self,
        ack_type: &MessageBody,
        recipient: &NodeContact,
    ) -> Result<()> {
        todo!();
    }
}
