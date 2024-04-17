#![allow(unused_variables)]

use crate::{clients::base::BaseClient, utils::message::Message};
use urlencoding;

/// Client for [11/WAKU2-RELAY](https://github.com/vacp2p/rfc-index/blob/main/waku/standards/core/11/relay.md) operations.
pub struct RelayClient {
    base: BaseClient,
}

impl RelayClient {
    pub fn new(base: BaseClient) -> Self {
        RelayClient { base }
    }

    /// Send a subscribed message.
    pub async fn send_message(&self, message: Message) -> Result<(), Box<dyn std::error::Error>> {
        let message = serde_json::json!(message);
        self.base.post("relay/v1/auto/messages", message).await?;
        Ok(())
    }

    /// Get subscribed messages with a topic.
    pub async fn get_messages(
        &self,
        content_topic: &str,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let topic = urlencoding::encode(content_topic).to_string();
        println!("topic: {}", topic);
        let res = self
            .base
            .get(&format!("relay/v1/auto/messages/{}", topic), None)
            .await?;

        let msgs = res.json().await?;
        Ok(msgs)
    }

    /** Subscribe to a pub-sub topic. */
    pub async fn subscribe(
        &self,
        pubsub_topics: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let res = self
            .base
            .post(
                "relay/v1/subscriptions",
                serde_json::json!(pubsub_topics.as_slice()),
            )
            .await?;

        let txt = res.text().await?;
        if txt == "OK" {
            Ok(())
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                txt,
            )))
        }
    }

    /// Unsubscribe from a pub-sub topic.
    pub async fn unsubscribe(
        &self,
        pubsub_topics: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.base
            .delete("relay/v1/subscriptions", serde_json::json!(pubsub_topics))
            .await?;
        Ok(())
    }

    /// Send a subscribed message.
    pub async fn send_subscribed_message(
        &self,
        pubsub_topic: &str,
        message: Message,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.base
            .post(
                &format!("relay/v1/messages/{}", pubsub_topic),
                serde_json::json!(message),
            )
            .await?;
        Ok(())
    }

    /** Get subscribed messages with a topic. */
    pub async fn get_subscribed_messages(
        &self,
        pubsub_topic: &str,
    ) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
        let res = self
            .base
            .get(&format!("relay/v1/messages/{}", pubsub_topic), None)
            .await?;

        if res.status().is_success() {
            let msgs = res.json().await?;
            Ok(msgs)
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                res.status().to_string(),
            )))
        }
    }
}
