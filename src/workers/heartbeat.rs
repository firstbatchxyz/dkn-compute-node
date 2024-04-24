use crate::{
    node::DriaComputeNode,
    utils::{
        crypto::sha256hash,
        message::{create_content_topic, WakuMessage},
    },
};

use serde::{Deserialize, Serialize};

const TOPIC: &str = "heartbeat";
const SLEEP_MILLIS: u64 = 500;

/// Heartbeat Payload contains just the `uuid` as a string.
/// This uuid can be used as the content topic to respond with a signature for that heartbeat.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String,
}

pub fn heartbeat_worker(mut node: DriaComputeNode) -> tokio::task::JoinHandle<()> {
    let topic: String = create_content_topic(TOPIC);
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);

    tokio::spawn(async move {
        match node.subscribe_topic(topic.clone()).await {
            Ok(_) => {
                println!("Subscribed to {}", topic);
            }
            Err(e) => {
                println!("Error subscribing to {}", e);
            }
        }

        loop {
            let mut message: Option<WakuMessage> = None;
            match node
                .process_topic(topic.clone(), |_, messages| {
                    println!("Heartbeats: {:?}", messages);

                    // we only care about the latest heartbeat
                    if let Some(message) = messages.last() {
                        let uuid = message
                            .parse_payload::<HeartbeatPayload>()
                            .expect("TODO TODO") // TODO: error handling
                            .uuid;
                        let signature = node.sign_bytes(&sha256hash(uuid.as_bytes()));

                        Some(WakuMessage::new(signature, &uuid, false))
                    } else {
                        None
                    }
                })
                .await
            {
                Ok(message_) => {
                    message = message_;
                }
                Err(error) => {
                    println!("Error processing heartbeat: {:?}", error);
                }
            }

            // send message
            if let Some(message) = message {
                node.waku
                    .relay
                    .send_message(message)
                    .await
                    .expect("TODO TODO");
            }

            tokio::time::sleep(sleep_amount).await;
        }
    })
}
