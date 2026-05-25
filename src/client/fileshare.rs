use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::client::DolomedesClient;
use crate::client::messages::{Message, MessageType};
use crate::client::routing::FileId;
use crate::kadem::{Kademlia, NodeContact, NodeId};
use anyhow::{Result, bail, ensure};
use binary_heap_plus::BinaryHeap;
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{FuturesUnordered, StreamExt};
use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};
use tokio::task::JoinSet;

fn order_nodes_by_xor_distance(file: FileId, a: &NodeContact, b: &NodeContact) -> Ordering {
    Kademlia::xor_distance(file, b.node_id)
        .cmp(&Kademlia::xor_distance(file, a.node_id))
        .then_with(|| b.node_id.cmp(&a.node_id))
}

impl DolomedesClient {
    const ERR_DOESNT_OWN_FILE: i64 = 1;
    const CHUNK_SIZE_BYTES: i64 = 1024 * 64;
    const MAX_CONCURRENT_CHUNK_REQUESTS: usize = 5;
    //TODO: I'm concerned that nodes will converge on similar k-buckets for a file and if it's popular, we could have an
    // extremely popular file effectively capped at 8 seeders -- find a way to fix this
    // (maybe if we're unable to handle a request we can return a node that the requester is unlikely to have (eg our newest node?)
    pub async fn handle_chunk_request(file: FileId) -> Result<()> {
        todo!()
    }

    //TODO: should impement these functions so that they get a vec of mutexes around the k buckets that they should be
    // querying rather than needing a full mutable reference to the routing table, as we won't be able to have multiple threads up at once
    // all mutably borrowing the routing table

    // just a note for future implementation, the smartest design is probably one where a node can request chunks of arbitrary
    // size from owners and they can set their own rate limits rather than requesting full files.
    pub async fn download_file(
        &self,
        file_id: FileId,
        path: PathBuf,
        destination: PathBuf,
    ) -> Result<()> {
        let seeders = self
            .find_seeders(
                file_id,
                self.routing_table
                    .bucket_for(file_id)
                    .lock()
                    .unwrap()
                    .clone()
                    .into_iter(),
            )
            .await?;

        ensure!(!seeders.is_empty());

        let metadata_response = self
            .send(
                &Message::new(
                    MessageType::GetFileMetadata { file_id },
                    self.node_id,
                    &self.signing_key,
                ),
                seeders.first().unwrap(),
            )
            .await?;

        let file_size: usize;
        if let MessageType::FileMetadata {
            file_id: metadata_file_id,
            file_size: metadata_file_size,
            file_name: _,
        } = MessageType::from_payload(metadata_response.payload)
        {
            ensure!(metadata_file_id == file_id);
            file_size = metadata_file_size.try_into().unwrap();
        } else {
            bail!(
                "seeder {} failed to respond properly",
                seeders.first().unwrap().node_id
            )
        }

        let chunk_size_bytes = Self::CHUNK_SIZE_BYTES as usize;
        let total_chunks = file_size.div_ceil(chunk_size_bytes);
        let mut chunk = 0;
        let mut destination_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(destination)?;

        let mut seeders = seeders.iter().cycle();

        let mut requests: FuturesUnordered<BoxFuture<'_, Result<(usize, Box<[u8]>)>>> =
            FuturesUnordered::new();
        while chunk < total_chunks {
            let seeder = seeders.next().unwrap().clone();
            let chunk_request = Message::new(
                MessageType::GetChunk {
                    chunk_index: chunk.try_into().expect("chunk index over 32 bits"),
                    chunk_size: chunk_size_bytes.try_into().unwrap(),
                    file_id: file_id,
                },
                self.node_id,
                &self.signing_key,
            );
            if requests.len() == Self::MAX_CONCURRENT_CHUNK_REQUESTS {
                let (chunk_index, chunk_data) = requests
                    .next()
                    .await
                    .unwrap()
                    .expect("TODO: implement timeouts and retry");
                destination_file.seek(SeekFrom::Start(
                    (chunk_index * chunk_size_bytes)
                        .try_into()
                        .expect("file offset overflow"),
                ))?;
                destination_file.write_all(&chunk_data)?;
            }

            //TODO: finish

            requests.push(
                async move {
                    let response = self.send(&chunk_request, &seeder).await?;
                    match MessageType::from_payload(response.payload) {
                        MessageType::Chunk {
                            chunk_index,
                            chunk_size,
                            file_id,
                            data,
                        } => {
                            //TODO: ensure! chunk index sixe and file id matchup i'm just lazy an llm can do this
                            return Ok((chunk_index.try_into().unwrap(), data));
                        }
                        _ => {
                            bail!("invalid response")
                        }
                    }
                }
                .boxed(),
            );

            chunk += 1;
        }

        //TODO: we need a smarter and more secure file sharing method later on. this is a hack to get version 0.0.1 out
        todo!()
    }

    async fn find_seeders(
        &self,
        file: FileId,
        candidates: impl Iterator<Item = NodeContact>,
    ) -> Result<Vec<NodeContact>> {
        // priority q of nodes by xor distance from target. nodes know more nodes closer to themselves
        // so we want to keep querying the closest node we know to the file until one has it
        let mut seen: HashSet<NodeId> = HashSet::new();
        let mut nodes = BinaryHeap::new_by(|a: &NodeContact, b: &NodeContact| {
            order_nodes_by_xor_distance(file, a, b)
        });
        nodes.extend(candidates.filter(|contact| seen.insert(contact.node_id)));

        while let Some(node) = nodes.pop() {
            let ownership_check = Message::new(
                MessageType::GetSeeders { file_id: file },
                self.node_id,
                &self.signing_key,
            );
            match MessageType::from_payload(self.send(&ownership_check, &node).await?.payload) {
                MessageType::Error { code } => {
                    ensure!(code == Self::ERR_DOESNT_OWN_FILE);
                }
                MessageType::Nodes { nodes } => {
                    return Ok(nodes);
                }
                _ => {
                    self.routing_table.evict(node.node_id).await;
                }
            }

            let find_owners = Message::new(
                MessageType::GetOwners { file_id: file },
                self.node_id,
                &self.signing_key,
            );
            let response = self.send(&find_owners, &node).await?;

            match MessageType::from_payload(response.payload) {
                MessageType::Nodes {
                    nodes: closer_nodes,
                } => {
                    let bucket = self.routing_table.bucket_for(node.node_id);
                    self.insert_with_ping(bucket, &node).await;
                    for node in closer_nodes {
                        if seen.insert(node.node_id) {
                            nodes.push(node);
                        }
                    }
                }
                _ => {
                    self.routing_table.evict(node.node_id).await;
                    continue;
                }
            }
        }

        bail!(
            "routing table sputtered out while looking for file {file}, did nukes drop? am i just dumb?"
        );
    }
}
