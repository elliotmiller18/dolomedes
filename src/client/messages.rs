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

    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
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
        let mut buf = vec![self.discriminant()];
        match self {
            Self::DeclareSeed { file_id } => buf.extend_from_slice(&file_id.to_be_bytes()),
            Self::GetNode { node } => buf.extend_from_slice(&node.to_be_bytes()),
            Self::GetOwners { file_id } => buf.extend_from_slice(&file_id.to_be_bytes()),
            Self::Nodes { nodes } => {
                buf.extend_from_slice(
                    &u32::try_from(nodes.len())
                        .expect("too many nodes")
                        .to_le_bytes(),
                );
                nodes.iter().for_each(|n| {
                    buf.extend_from_slice(&n.node_id.to_be_bytes());
                    match n.ip {
                        IpAddr::V4(v4) => {
                            buf.push(4);
                            buf.extend_from_slice(&v4.octets());
                        }
                        IpAddr::V6(v6) => {
                            buf.push(6);
                            buf.extend_from_slice(&v6.octets());
                        }
                    }
                    buf.extend_from_slice(&n.port.to_le_bytes());
                });
            }
            Self::JoinNetwork {
                port,
                nonce,
                verifying_key,
            } => {
                buf.extend_from_slice(&port.to_le_bytes());
                buf.extend_from_slice(&nonce.to_be_bytes());
                buf.extend_from_slice(verifying_key.as_bytes());
            }
            Self::GetFileMetadata { file_id } => buf.extend_from_slice(&file_id.to_be_bytes()),
            Self::FileMetadata {
                file_id,
                file_size,
                file_name,
            } => {
                buf.extend_from_slice(&file_id.to_be_bytes());
                buf.extend_from_slice(&file_size.to_le_bytes());
                buf.extend_from_slice(
                    &u32::try_from(file_name.len())
                        .expect("file name too large")
                        .to_le_bytes(),
                );
                buf.extend_from_slice(file_name.as_bytes());
            }
            Self::GetChunk {
                chunk_index,
                chunk_size,
                file_id,
            } => {
                buf.extend_from_slice(&chunk_index.to_le_bytes());
                buf.extend_from_slice(&chunk_size.to_le_bytes());
                buf.extend_from_slice(&file_id.to_be_bytes());
            }
            Self::Chunk {
                chunk_index,
                chunk_size,
                file_id,
                data,
            } => {
                buf.extend_from_slice(&chunk_index.to_le_bytes());
                buf.extend_from_slice(&chunk_size.to_le_bytes());
                buf.extend_from_slice(&file_id.to_be_bytes());
                buf.extend_from_slice(
                    &u32::try_from(data.len())
                        .expect("chunk too large")
                        .to_le_bytes(),
                );
                buf.extend_from_slice(&data);
            }
            Self::GetSeeders { file_id } => buf.extend_from_slice(&file_id.to_be_bytes()),
            Self::Error { code } => buf.extend_from_slice(&code.to_le_bytes()),
            Self::Ping | Self::PingAck | Self::SeedAck | Self::JoinAck | Self::InvalidMessage => {}
        }
        buf.into_boxed_slice()
    }

    fn from_payload(payload: &[u8]) -> Result<Self> {
        let mut buf = payload;
        match take_bytes(&mut buf, 1)?[0] {
            0x00 => Ok(Self::Ping),
            0x01 => Ok(Self::PingAck),
            0x02 => Ok(Self::DeclareSeed {
                file_id: U256::from_be_slice(take_bytes(&mut buf, 32)?),
            }),
            0x03 => Ok(Self::SeedAck),
            0x04 => Ok(Self::GetNode {
                node: U256::from_be_slice(take_bytes(&mut buf, 32)?),
            }),
            0x05 => Ok(Self::GetOwners {
                file_id: U256::from_be_slice(take_bytes(&mut buf, 32)?),
            }),
            0x06 => {
                let count: usize = u32::from_le_bytes(take_bytes(&mut buf, 4)?.try_into().unwrap())
                    .try_into()
                    .unwrap();
                let nodes = (0..count)
                    .map(|_| -> Result<NodeContact> {
                        let node_id = U256::from_be_slice(take_bytes(&mut buf, 32)?);
                        let ip = match take_bytes(&mut buf, 1)?[0] {
                            4 => IpAddr::V4(Ipv4Addr::from(
                                <[u8; 4]>::try_from(take_bytes(&mut buf, 4)?).unwrap(),
                            )),
                            6 => IpAddr::V6(Ipv6Addr::from(
                                <[u8; 16]>::try_from(take_bytes(&mut buf, 16)?).unwrap(),
                            )),
                            tag => bail!("unknown IP address family tag: {tag}"),
                        };
                        let port = u16::from_le_bytes(take_bytes(&mut buf, 2)?.try_into().unwrap());
                        Ok(NodeContact { node_id, ip, port })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Self::Nodes { nodes })
            }
            0x07 => {
                let port = u16::from_le_bytes(take_bytes(&mut buf, 2)?.try_into().unwrap());
                let nonce = U256::from_be_slice(take_bytes(&mut buf, 32)?);
                let vk_bytes: [u8; 32] = take_bytes(&mut buf, 32)?.try_into().unwrap();
                let verifying_key =
                    VerifyingKey::from_bytes(&vk_bytes).context("invalid verifying key")?;
                Ok(Self::JoinNetwork {
                    port,
                    nonce,
                    verifying_key,
                })
            }
            0x08 => Ok(Self::JoinAck),
            0x09 => Ok(Self::GetFileMetadata {
                file_id: U256::from_be_slice(take_bytes(&mut buf, 32)?),
            }),
            0x0A => {
                let file_id = U256::from_be_slice(take_bytes(&mut buf, 32)?);
                let file_size = u64::from_le_bytes(take_bytes(&mut buf, 8)?.try_into().unwrap());
                let name_len: usize =
                    u32::from_le_bytes(take_bytes(&mut buf, 4)?.try_into().unwrap())
                        .try_into()
                        .unwrap();
                let file_name = String::from_utf8(take_bytes(&mut buf, name_len)?.to_vec())
                    .context("invalid UTF-8 in file name")?;
                Ok(Self::FileMetadata {
                    file_id,
                    file_size,
                    file_name,
                })
            }
            0x0B => Ok(Self::GetChunk {
                chunk_index: u32::from_le_bytes(take_bytes(&mut buf, 4)?.try_into().unwrap()),
                chunk_size: u64::from_le_bytes(take_bytes(&mut buf, 8)?.try_into().unwrap()),
                file_id: U256::from_be_slice(take_bytes(&mut buf, 32)?),
            }),
            0x0C => {
                let chunk_index = u32::from_le_bytes(take_bytes(&mut buf, 4)?.try_into().unwrap());
                let chunk_size = u64::from_le_bytes(take_bytes(&mut buf, 8)?.try_into().unwrap());
                let file_id = U256::from_be_slice(take_bytes(&mut buf, 32)?);
                let data_len: usize =
                    u32::from_le_bytes(take_bytes(&mut buf, 4)?.try_into().unwrap())
                        .try_into()
                        .unwrap();
                let data = take_bytes(&mut buf, data_len)?.to_vec().into_boxed_slice();
                Ok(Self::Chunk {
                    chunk_index,
                    chunk_size,
                    file_id,
                    data,
                })
            }
            0x0D => Ok(Self::GetSeeders {
                file_id: U256::from_be_slice(take_bytes(&mut buf, 32)?),
            }),
            0xFE => Ok(Self::InvalidMessage),
            0xFF => Ok(Self::Error {
                code: i64::from_le_bytes(take_bytes(&mut buf, 8)?.try_into().unwrap()),
            }),
            tag => bail!("unknown message discriminant: {tag:#04x}"),
        }
    }
}

fn take_bytes<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    ensure!(buf.len() >= n, "unexpected end of message");
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

impl DolomedesClient {
    pub(crate) async fn send(&self, message: &Message, recipient: &NodeContact) -> Result<Message> {
        //TODO: down the line MSG_ZEROCOPY might be useful for seeding, as we're sending the same or an almost identical packet
        // over and over to different sources.

        //TODO: centralize routing table updates here so handlers don't need to worry about semantics.
        // this means insertions as well as evictions.

        // also im thinking when this errors it's like an OS or Network error, not just
        // that we couldn't find the sender. maybe return Result<bool>?
        todo!();
    }

    pub(crate) async fn fire(&self, message: &Message, recipient: &NodeContact) -> Result<()> {
        todo!()
    }

    pub(crate) async fn send_discriminant(
        &self,
        message: MessageBody,
        recipient: &NodeContact,
    ) -> Result<()> {
        todo!()
    }
}
