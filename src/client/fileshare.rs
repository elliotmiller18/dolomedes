use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::client::DolomedesClient;
use crate::client::messages::{Message, MessageBody};
use crate::client::routing::FileId;
use crate::kadem::NodeContact;
use anyhow::{Result, bail, ensure};
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{FuturesUnordered, StreamExt};

impl DolomedesClient {
    pub(crate) const ERR_DOESNT_OWN_FILE: i64 = 1;
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
        let bucket = self
            .routing_table
            .bucket_for(file_id)
            .lock()
            .unwrap()
            .clone();
        let seeders = self.find_seeders(file_id, bucket.into_iter()).await?;

        ensure!(!seeders.is_empty());

        //TODO: retry with more seeders
        let metadata_response = self
            .send(
                &Message::new(
                    MessageBody::GetFileMetadata { file_id },
                    self.node_id,
                    &self.signing_key,
                ),
                seeders.first().unwrap(),
            )
            .await?;

        let file_size: usize;
        if let MessageBody::FileMetadata {
            file_id: metadata_file_id,
            file_size: metadata_file_size,
            file_name: _,
        } = MessageBody::from_payload(metadata_response.payload)
        {
            ensure!(metadata_file_id == file_id);
            file_size = metadata_file_size.try_into().unwrap();
        } else {
            bail!(
                "seeder {} failed to respond properly",
                seeders.first().unwrap().node_id
            )
        }

        let chunk_size_bytes = Self::CHUNK_SIZE_BYTES.try_into().unwrap();
        let total_chunks = file_size.div_ceil(chunk_size_bytes);
        let final_chunk_size = file_size % chunk_size_bytes;
        let mut chunk = 0;
        let mut destination_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(destination)?;

        let mut seeders = seeders.iter().cycle();

        let mut requests: FuturesUnordered<BoxFuture<'_, Result<(usize, Box<[u8]>)>>> =
            FuturesUnordered::new();
        loop {
            if requests.len() == Self::MAX_CONCURRENT_CHUNK_REQUESTS {
                let (chunk_index, chunk_data) = requests
                    .next()
                    .await
                    .unwrap()
                    .expect("TODO: implement timeouts");
                destination_file.seek(SeekFrom::Start(
                    (chunk_index * chunk_size_bytes)
                        .try_into()
                        .expect("file offset overflow"),
                ))?;
                if chunk_data.len() == Self::CHUNK_SIZE_BYTES.try_into().unwrap() {
                    destination_file.write_all(&chunk_data)?;
                } else if chunk_data.len() == final_chunk_size {
                    ensure!(
                        chunk == total_chunks - 1,
                        "extra chunk with final chunk size"
                    );
                    destination_file.write_all(&chunk_data)?;
                } else {
                    bail!("chunk with invalid size");
                }
            }

            if chunk == total_chunks {
                continue;
            }

            let seeder = seeders.next().unwrap().clone();
            let chunk_request = Message::new(
                MessageBody::GetChunk {
                    chunk_index: chunk.try_into().expect("chunk index over 32 bits"),
                    chunk_size: chunk_size_bytes.try_into().unwrap(),
                    file_id: file_id,
                },
                self.node_id,
                &self.signing_key,
            );

            requests.push(
                async move {
                    //TODO: put retry logic here. maybe Iterator<Item=&NodeContact> ?
                    let response = self.send(&chunk_request, &seeder).await?;
                    match MessageBody::from_payload(response.payload) {
                        MessageBody::Chunk {
                            chunk_index,
                            chunk_size,
                            file_id,
                            data,
                        } => {
                            //TODO: ensure! chunk index sixe and file id matchup i'm just lazy an llm can do
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

    pub async fn serve_chunk(
        &self,
        requester: &NodeContact,
        chunk_index: u32,
        chunk_size: u64,
        file_id: FileId,
        path: PathBuf,
    ) -> Result<()> {
        use std::io::Read;

        let mut file = File::open(path).unwrap();
        file.seek(SeekFrom::Start(chunk_size * (u64::from(chunk_index))))?;

        let mut data = vec![0; chunk_size.try_into().unwrap()].into_boxed_slice();
        file.read(&mut data)?;

        let response = Message::new(
            MessageBody::Chunk {
                chunk_index,
                chunk_size,
                file_id,
                data,
            },
            self.node_id,
            &self.signing_key,
        );
        self.send(&response, requester).await?;

        Ok(())
    }
}
