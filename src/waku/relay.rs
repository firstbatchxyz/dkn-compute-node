use crate::waku::BaseClient;
use urlencoding;

use super::message::WakuMessage;

/// Client for [11/WAKU2-RELAY](https://github.com/vacp2p/rfc-index/blob/main/waku/standards/core/11/relay.md) operations.
///
/// The relay client is used to send and receive messages to Waku network. It works as follows:
///
/// 1. A node subscribes to a content topic
/// 2. Nodes that are subscribed to the same content topic can send and receive messages via the network.
/// 3. On termination, the node unsubscribes from the content topic.
#[derive(Debug, Clone)]
pub struct RelayClient {
    base: BaseClient,
}

// TODO: dont create content topic outside and pass it in here, have each function create the parameter itself.

impl RelayClient {
    pub fn new(base: BaseClient) -> Self {
        RelayClient { base }
    }

    /// Send a message.
    pub async fn send_message(
        &self,
        message: WakuMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Sending message:\n{:?}", message);
        let message = serde_json::json!(message);
        self.base.post("relay/v1/auto/messages", message).await?;

        Ok(())
    }

    /// Get messages with a given content topic.
    ///
    /// The content topic must have been subscribed to before.
    pub async fn get_messages(
        &self,
        content_topic: &str,
    ) -> Result<Vec<WakuMessage>, Box<dyn std::error::Error>> {
        log::info!("Polling {}", content_topic);
        let content_topic_encoded = urlencoding::encode(content_topic).to_string();
        let res = self
            .base
            .get(
                &format!("relay/v1/auto/messages/{}", content_topic_encoded),
                None,
            )
            .await?;

        // parse body
        let msgs = res.json().await?;

        Ok(msgs)
    }

    /// Subscribe to a topic.
    pub async fn subscribe(&self, content_topic: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Subscribing to {}", content_topic);
        self.base
            .post(
                "relay/v1/auto/subscriptions",
                serde_json::json!(vec![content_topic]),
            )
            .await?;

        Ok(())
    }

    /// Unsubscribe from a content topic.
    pub async fn unsubscribe(&self, content_topic: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Unsubscribing from {}", content_topic);
        self.base
            .delete(
                "relay/v1/auto/subscriptions",
                serde_json::json!(vec![content_topic]),
            )
            .await?;

        Ok(())
    }
}
