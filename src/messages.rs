use crypto_bigint::U256;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};

use crate::proto::FileId;

#[derive(Clone)]
pub struct Message {
    payload: Box<[u8]>,
    signature: Signature,
    timestamp: u64,
}

impl Message {
    pub fn new(message_type: MessageType, signing_key: SigningKey) -> Self {
        let timestamp: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // here we return a Self with the signature of sign(payload.as_bytes() + timestamp)
        // let payload_byte = payload.

        // we also need to append the node_id to the end of the payload

        todo!();
    }

    pub fn verify(&self, verifying_key: VerifyingKey) -> bool {
        todo!()
    }
}

pub enum MessageType {
    // port, nonce, verifying key
    JoinNetwork(u16, U256, VerifyingKey),
    JoinAck,
    JoinReject,
    // file size (bytes), file id, file name, file data (should be small, always a nonce rn)
    Store(u16, FileId, Option<String>, Box<u8>),
    RequestFile(FileId),
    // chunk index (0-indexed), chunk size (bytes), file id, file data (maybe asser that it's the size of arg 2?)
    Chunk(u16, u16, FileId, Box<u8>),
    // chunk index (0-indexed), FileId
    ChunkAck(u16, FileId),
}
