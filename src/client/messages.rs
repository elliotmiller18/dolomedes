use crate::client::DolomedesClient;
use anyhow::{Context, Result, bail, ensure};
use crypto_bigint::U256;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::client::routing::FileId;
use crate::kadem::{NodeContact, NodeId};

#[derive(Clone)]
pub struct Message {
    pub node_id: NodeId,
    payload: Box<[u8]>,
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

    pub fn from_payload(payload: &[u8]) -> Result<Self> {
        //TODO: call verify() here
        todo!()
    }

    pub fn to_payload(&self) -> Box<[u8]> {
        let mut buf = Vec::with_capacity(self.payload.len() + size_of::<Signature>() + 8);
        buf.extend_from_slice(&self.payload);
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        //TODO: endianness issues on this to_bytes?
        buf.extend_from_slice(&self.signature.to_bytes());
        buf.into_boxed_slice()
    }

    fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        let to_verify = Self::signable_payload(&self.payload, self.timestamp);
        verifying_key
            .verify_strict(&to_verify, &self.signature)
            .is_ok()
    }

    pub fn body(self) -> MessageBody {
        MessageBody::from_payload(&self.payload).expect("invalid message payload")
    }

    fn signable_payload(payload: &[u8], timestamp: u64) -> Box<[u8]> {
        let mut buf = Vec::with_capacity(payload.len() + 8);
        buf.extend_from_slice(payload);
        buf.extend_from_slice(&timestamp.to_le_bytes());
        buf.into_boxed_slice()
    }
}

// all file sizes in bytes
pub enum MessageBody {
    Ping,
    PingAck,
    DeclareSeed {
        file_id: FileId,
    },
    SeedAck,
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
    //NOTE: discriminant and to/from payload are all vibe coded, they look fine but double check on verification
    fn discriminant(&self) -> u8 {
        const PING: u8 = 0x00;
        const PING_ACK: u8 = 0x01;
        const DECLARE_SEED: u8 = 0x02;
        const SEED_ACK: u8 = 0x03;
        const GET_NODE: u8 = 0x04;
        const GET_OWNERS: u8 = 0x05;
        const NODES: u8 = 0x06;
        const JOIN_NETWORK: u8 = 0x07;
        const JOIN_ACK: u8 = 0x08;
        const GET_FILE_METADATA: u8 = 0x09;
        const FILE_METADATA: u8 = 0x0A;
        const GET_CHUNK: u8 = 0x0B;
        const CHUNK: u8 = 0x0C;
        const GET_SEEDERS: u8 = 0x0D;
        const INVALID_MESSAGE: u8 = 0xFE;
        const ERROR: u8 = 0xFF;

        match self {
            Self::Ping => PING,
            Self::PingAck => PING_ACK,
            Self::DeclareSeed { .. } => DECLARE_SEED,
            Self::SeedAck => SEED_ACK,
            Self::GetNode { .. } => GET_NODE,
            Self::GetOwners { .. } => GET_OWNERS,
            Self::Nodes { .. } => NODES,
            Self::JoinNetwork { .. } => JOIN_NETWORK,
            Self::JoinAck => JOIN_ACK,
            Self::GetFileMetadata { .. } => GET_FILE_METADATA,
            Self::FileMetadata { .. } => FILE_METADATA,
            Self::GetChunk { .. } => GET_CHUNK,
            Self::Chunk { .. } => CHUNK,
            Self::GetSeeders { .. } => GET_SEEDERS,
            Self::InvalidMessage => INVALID_MESSAGE,
            Self::Error { .. } => ERROR,
        }
    }

    pub fn to_payload(self) -> Box<[u8]> {
        todo!()
    }

    fn from_payload(payload: &[u8]) -> Result<Self> {
        todo!()
    }
}

fn take_bytes<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    ensure!(buf.len() >= n, "unexpected end of message");
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

impl DolomedesClient {
    pub(crate) async fn send(&self, message: &Message, tx: &mut quinn::SendStream) -> Result<()> {
        let payload = message.to_payload();
        tx.write_all(&payload).await.unwrap();
        //TODO: down the line MSG_ZEROCOPY might be useful for seeding, as we're sending the same or an almost identical packet
        // over and over to different sources.
        Ok(())
    }

    pub(crate) async fn recv(&self, rx: &mut quinn::RecvStream) -> Result<Message> {
        let mut buf = Vec::new();
        rx.read(&mut buf).await.unwrap();
        Message::from_payload(&buf)
    }
}
