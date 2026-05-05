use crypto_bigint::U256;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::client::request::FileId;

#[derive(Clone)]
pub struct Message {
    pub payload: Box<[u8]>,
    signature: Signature,
    timestamp: u64,
}

impl Message {
    pub fn new(message_type: MessageType, signing_key: &SigningKey) -> Self {
        let payload = message_type.to_payload();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let to_sign = Self::signable_payload(&payload, timestamp);

        Self {
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

pub enum MessageType {
    // port, nonce, verifying key
    JoinNetwork(u16, U256, VerifyingKey),
    JoinAck,
    JoinReject,
    // file size (bytes), file id, file data (should be small, always a nonce rn)
    Store(u32, FileId, Box<[u8]>),
    StoreAck,
    // chunk index (0-indexed), chunk size (bytes), file id
    ChunkRequest(u32, u32, FileId),
    // chunk index (0-indexed), chunk size (bytes), file id, file data (maybe asser that it's the size of arg 2?)
    Chunk(u32, u32, FileId, Box<[u8]>),
    // chunk index (0-indexed), FileId
    ChunkAck(u32, FileId),
    InvalidMessage,
}

impl MessageType {
    pub fn to_payload(self) -> Box<[u8]> {
        todo!()
    }

    pub fn from_payload(payload: Box<[u8]>) -> Self {
        todo!()
    }
}
