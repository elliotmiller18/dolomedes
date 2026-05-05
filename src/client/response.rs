use anyhow::Result;

/// This file is handles the all responses the DolomedesClients is expected to provide
use crate::client::DolomedesClient;
use crate::client::request::FileId;
use crate::kadem::NodeContact;

impl<F> DolomedesClient<F>
where
    F: AsyncFn(&NodeContact) -> bool,
{
    //TODO: I'm concerned that nodes will converge on similar k-buckets for a file and if it's popular, we could have an
    // extremely popular file effectively capped at 8 seeders -- find a way to fix this
    // (maybe if we're unable to handle a request we can return a node that the requester is unlikely to have (eg our newest node?)
    pub async fn handle_chunk_request(file: FileId) -> Result<()> {
        todo!()
    }
}
