use crate::{node::DriaComputeNode, utils::crypto::sha256hash, waku::message::WakuMessage};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

const TOPIC: &str = "heartbeat";
const SLEEP_MILLIS: u64 = 1000;

/// # Heartbeat Payload
///
/// A heartbeat is a message sent by a node to indicate that it is alive. Dria nodes request
/// a heartbeat with a unique identifier, and the requester node will sign the identifier and send the signature back to a topic
/// identified with the `uuid`.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String,
    deadline: u128,
}

pub fn heartbeat_worker(
    node: DriaComputeNode,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);

    tokio::spawn(async move {
        match node.subscribe_topic(TOPIC).await {
            Ok(_) => {
                log::info!("Subscribed to {}", TOPIC);
            }
            Err(e) => {
                log::error!("Error subscribing to {}", e);
                return;
            }
        }

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    node.unsubscribe_topic(TOPIC).await
                        .expect("TODO TODO");
                    break;
                }
                _ = tokio::time::sleep(sleep_amount) => {
                    let mut msg_to_send: Option<WakuMessage> = None; 
                    if let Ok(messages) = node.process_topic(TOPIC, true).await {

                        // we only care about the latest heartbeat
                        if let Some(message) = messages.last() {
                            log::info!("Received: {:?}", message);

                            let body = message
                                .parse_payload::<HeartbeatPayload>(true)
                                .expect("TODO TODO");
                            let uuid = body.uuid;
                            let signature = node.sign_bytes(&sha256hash(uuid.as_bytes()));

                            msg_to_send = Some(WakuMessage::new(signature, &uuid));
                        }
                    } else {
                        log::error!("Error processing topic {}", TOPIC);
                    }

                    // send message
                    if let Some(message) = msg_to_send {
                        node.send_once_message(message).await
                            .expect("TODO TODO");
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{config::defaults::DEFAULT_DKN_ADMIN_PUBLIC_KEY, waku::message::WakuMessage};
    use libsecp256k1::PublicKey;

    use super::HeartbeatPayload;

    #[test]
    fn test_heartbeat_payload() {
        let pk = PublicKey::parse_compressed(DEFAULT_DKN_ADMIN_PUBLIC_KEY).unwrap();
        let message = WakuMessage { 
            payload: "Y2RmODcyNDlhY2U3YzQ2MDIzYzNkMzBhOTc4ZWY3NjViMWVhZDlmNWJhMDUyY2MxMmY0NzIzMjQyYjc0YmYyODFjMDA1MTdmMGYzM2VkNTgzMzk1YWUzMTY1ODQ3NWQyNDRlODAxYzAxZDE5MjYwMDM1MTRkNzEwMThmYTJkNjEwMXsidXVpZCI6ICI4MWE2M2EzNC05NmM2LTRlNWEtOTliNS02YjI3NGQ5ZGUxNzUiLCAiZGVhZGxpbmUiOiAxNzE0MTI4NzkyfQ==".to_string(), 
            content_topic: "/dria/0/heartbeat/proto".to_string(), 
            version: 0, 
            timestamp: 1714129073557846272, 
            ephemeral: false 
        };

        assert!(message.is_signed(&pk).unwrap());

        let obj = message.parse_payload::<HeartbeatPayload>(true).unwrap();
        assert_eq!(obj.uuid, "81a63a34-96c6-4e5a-99b5-6b274d9de175");
        assert_eq!(obj.deadline, 1714128792);
    }
}
