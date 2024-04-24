use crate::{
    node::DriaComputeNode,
    utils::{
        crypto::sha256hash,
        message::{create_content_topic, WakuMessage},
    },
};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

const TOPIC: &str = "heartbeat";
const SLEEP_MILLIS: u64 = 500;

/// Heartbeat Payload contains just the `uuid` as a string.
/// This uuid can be used as the content topic to respond with a signature for that heartbeat.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String,
}

pub fn heartbeat_worker(
    mut node: DriaComputeNode,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
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
            // tokio::select! {
            //         // Step 3: Using cloned token to listen to cancellation requests
            //     _ = cancellation.cancelled() => {
            //         // The token was cancelled, task can shut down
            //     }
            //     _ = tokio::time::sleep(std::time::Duration::from_secs(9999)) => {
            //         // Long work has completed
            //     }
            // }
            let mut msg_to_send: Option<WakuMessage> = None;
            if let Ok(messages) = node.process_topic(topic.clone()).await {
                println!("Heartbeats: {:?}", messages);

                // we only care about the latest heartbeat
                if let Some(message) = messages.last() {
                    let uuid = message
                        .parse_payload::<HeartbeatPayload>()
                        .expect("TODO TODO") // TODO: error handling
                        .uuid;
                    let signature = node.sign_bytes(&sha256hash(uuid.as_bytes()));

                    msg_to_send = Some(WakuMessage::new(signature, &uuid, false));
                }
            }

            // send message
            if let Some(message) = msg_to_send {
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
