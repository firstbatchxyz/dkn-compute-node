#![allow(unused_variables)]

use std::collections::HashMap;

use crate::{clients::base::BaseClient, utils::message::Message};
use serde::{Deserialize, Serialize};

/// Client for [13/WAKU2-STORE](https://github.com/vacp2p/rfc-index/blob/main/waku/standards/core/13/store.md) operations.
#[derive(Debug, Clone)]
pub struct StoreClient {
    base: BaseClient,
}

impl StoreClient {
    pub fn new(base: BaseClient) -> Self {
        StoreClient { base }
    }

    /// Get stored messages.
    pub async fn get_messages(
        &self,
        content_topic: &str,
        ascending: Option<bool>,
        page_size: Option<usize>,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let mut opts = HashMap::new();
        opts.insert("content_topics".to_string(), content_topic.to_string());
        opts.insert("page_size".to_string(), page_size.unwrap_or(60).to_string());
        opts.insert(
            "ascending".to_string(),
            ascending.unwrap_or(false).to_string(),
        );

        let res = self.base.get("store/v1/messages", Some(opts)).await?;
        let payload = res.json::<StoreResponse>().await?;
        let messages = payload.messages;
        Ok(messages)
    }
}

#[derive(Serialize, Deserialize)]
struct StoreResponse {
    messages: Vec<Message>,
    cursor: Cursor,
}

#[derive(Serialize, Deserialize)]
struct Cursor {
    pubsub_topic: String,
    sender_time: u128,
    store_time: u128,
    digest: Digest,
}

#[derive(Serialize, Deserialize)]
struct Digest {
    data: String,
}
