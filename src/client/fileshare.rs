use anyhow::Result;

/// This file is handles the all responses the DolomedesClients is expected to provide
use crate::client::DolomedesClient;
use crate::client::routing::FileId;

impl DolomedesClient {
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
    pub async fn request_file(&mut self, file: FileId) -> Result<()> {
        todo!()
    }
}
