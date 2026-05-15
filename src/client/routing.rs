use crate::kadem::NodeContact;
/// This file is handles the all requests the DolomedesClients can issue
use crate::{
    client::{
        DolomedesClient,
        messages::{Message, MessageType},
    },
    kadem::Kademlia,
};
use std::{collections::VecDeque, sync::Mutex};

use anyhow::{Result, ensure};
use crypto_bigint::U256;

pub type FileId = U256;
pub const POW_LEADING_ZEROES: usize = 24;

impl DolomedesClient {
    /// join the dolomedes network for the **first** time, or if your routing table is lost.
    pub async fn join_network(&mut self, genesis_nodes: Vec<NodeContact>) -> Result<()> {
        let pow_nonce: U256 =
            crate::pow::generate_entry_nonce(self.signing_key.verifying_key(), POW_LEADING_ZEROES);
        let join_message = Message::new(
            MessageType::JoinNetwork(self.port, pow_nonce, self.signing_key.verifying_key()),
            self.node_id,
            &self.signing_key,
        );

        for node in genesis_nodes {
            match MessageType::from_payload(self.send(&join_message, &node).await?.payload) {
                MessageType::JoinAck => {
                    self.routing_table.insert(node);
                }
                _ => {
                    tracing::warn!("genesis node {} failed to respond properly", node.node_id);
                }
            }
        }
        ensure!(!self.routing_table.is_empty());

        self.store(self.node_id, Box::from(pow_nonce.to_le_bytes().as_slice()))
            .await?;

        Ok(())
    }

    pub async fn store(&mut self, key: FileId, value: Box<[u8]>) -> Result<()> {
        //TODO: we need to also maintain seeder lists (lists of nodes actually seeding files)
        // here as the way it's supposed to work is that nodes near a value, so maybe we can add an
        // optional original sender field to the protocol
        let recipients = self.routing_table.store(key, value.as_ref(), true)?;
        let store_message = Message::new(
            MessageType::Store(32, self.node_id, value),
            self.node_id,
            &self.signing_key,
        );

        for node in recipients {
            let response = self.send(&store_message, &node).await;
            if response.is_ok_and(|message| {
                !matches!(
                    MessageType::from_payload(message.payload),
                    MessageType::StoreAck
                )
            }) {
                self.routing_table.evict(node.node_id);
            } else {
                let bucket = self.routing_table.bucket_for(node.node_id);
                self.update_bucket(bucket, &node).await;
            }
        }

        Ok(())
    }

    /// update bucket given that we've just recieved a nice response from contact
    async fn update_bucket(&self, bucket: &Mutex<VecDeque<NodeContact>>, contact: &NodeContact) {
        let mut bucket = bucket.lock().unwrap();

        if let Some(pos) = bucket
            .iter()
            .position(|known_contact| known_contact.node_id == contact.node_id)
        {
            // this implicitly allows for us to easily update ip addresses and ports in case of a quick reconfig,
            // allows for nice graceful disconnect/reconnect cause sometimes someone wants to turn on a vpn or
            // whatever
            bucket.remove(pos).unwrap();
            bucket.push_front(contact.clone());
            return;
        } else if bucket.len() < Kademlia::BUCKET_SIZE {
            bucket.push_front(contact.clone());
        } else {
            let evicted = bucket.pop_back().unwrap();
            if self.ping(&evicted).await {
                bucket.push_front(evicted);
            } else {
                bucket.push_front(contact.clone());
            }
        }
        assert!(bucket.len() <= Kademlia::BUCKET_SIZE);
    }

    //TODO: return Result<bool> when we implement writing kademlia to disk, cause an Err shouldn't cause the node
    // to be kicked from the table, only an Ok(false) should.
    async fn ping(&self, contact: &NodeContact) -> bool {
        let message = Message::new(MessageType::Ping, self.node_id, &self.signing_key);
        let response = self.send(&message, contact).await;
        response.is_ok_and(|message| {
            !matches!(
                MessageType::from_payload(message.payload),
                MessageType::PingAck
            )
        })
    }
}
