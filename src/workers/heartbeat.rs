use std::sync::Arc;
use std::time::Duration;

use crate::{node::DriaComputeNode, utils::crypto::sha256hash, waku::message::WakuMessage};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String,
    deadline: u128,
}

/// # Heartbeat
///
/// A heartbeat is a message sent by a node to indicate that it is alive. Dria nodes request
/// a heartbeat with a unique identifier, and the requester node will sign the identifier and send the signature back to a topic
/// identified with the `uuid`.
pub fn heartbeat_worker(
    node: Arc<DriaComputeNode>,
    topic: &'static str,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        node.subscribe_topic(topic).await;

        loop {
            tokio::select! {
                _ = node.cancellation.cancelled() => {
                    if let Err(e) = node.unsubscribe_topic(topic).await {
                        log::error!("Error unsubscribing from {}: {}\nContinuing anyway.", topic, e);
                    }
                    break;
                }
                _ = tokio::time::sleep(sleep_amount) => {
                    let messages = match node.process_topic(topic, true).await {
                        Ok(messages) => messages,
                        Err(e) => {
                            log::error!("Error processing topic {}: {}", topic, e);
                            continue;
                        }
                    };


                    // we only care about the latest heartbeat
                    if let Some(message) = messages.last() {
                        if node.is_busy() {
                            log::info!("Node is busy, skipping heartbeat.");
                            continue;
                        }


                        log::info!("Received: {}", message);

                        let message = match message.parse_payload::<HeartbeatPayload>(true) {
                            Ok(body) => {
                                let uuid = body.uuid;
                                let signature = node.sign_bytes(&sha256hash(uuid.as_bytes()));
                                WakuMessage::new(signature, &uuid)
                            }
                            Err(e) => {
                                log::error!("Error parsing payload: {}", e);
                                continue;
                            }
                        };


                        // send message
                        if let Err(e) = node.send_message_once(message).await {
                            log::error!("Error sending message: {}", e);
                        }

                    }



                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        config::constants::DEFAULT_DKN_ADMIN_PUBLIC_KEY,
        node::DriaComputeNode,
        utils::{
            crypto::{sha256hash, to_address},
            filter::FilterPayload,
        },
        waku::message::WakuMessage,
    };
    use fastbloom_rs::{FilterBuilder, Membership};
    use libsecp256k1::{recover, Message, PublicKey};

    use super::HeartbeatPayload;

    #[test]
    fn test_heartbeat_payload() {
        let pk = PublicKey::parse_compressed(DEFAULT_DKN_ADMIN_PUBLIC_KEY)
            .expect("Should parse public key");
        let message = WakuMessage {
            payload: "Y2RmODcyNDlhY2U3YzQ2MDIzYzNkMzBhOTc4ZWY3NjViMWVhZDlmNWJhMDUyY2MxMmY0NzIzMjQyYjc0YmYyODFjMDA1MTdmMGYzM2VkNTgzMzk1YWUzMTY1ODQ3NWQyNDRlODAxYzAxZDE5MjYwMDM1MTRkNzEwMThmYTJkNjEwMXsidXVpZCI6ICI4MWE2M2EzNC05NmM2LTRlNWEtOTliNS02YjI3NGQ5ZGUxNzUiLCAiZGVhZGxpbmUiOiAxNzE0MTI4NzkyfQ==".to_string(), 
            content_topic: "/dria/0/heartbeat/proto".to_string(), 
            version: 0,
            timestamp: 1714129073557846272,
            ephemeral: true
        };

        assert!(message.is_signed(&pk).expect("Should check signature"));

        let obj = message
            .parse_payload::<HeartbeatPayload>(true)
            .expect("Should parse payload");
        assert_eq!(obj.uuid, "81a63a34-96c6-4e5a-99b5-6b274d9de175");
        assert_eq!(obj.deadline, 1714128792);
    }

    /// This test demonstrates the process of heartbeat & task assignment.
    ///
    /// A heart-beat message is sent over the network by Admin Node, and compute node responds with a signature.
    #[test]
    fn test_heartbeat_and_task_assignment() {
        let node = DriaComputeNode::default();

        // a heartbeat message is signed and sent to Admin Node (via Waku network)
        let heartbeat_message = Message::parse(&sha256hash(b"sign-me"));
        let (heartbeat_signature, heartbeat_recid) = node.sign(&heartbeat_message);

        // admin recovers the address from the signature
        let recovered_public_key =
            recover(&heartbeat_message, &heartbeat_signature, &heartbeat_recid)
                .expect("Could not recover");
        assert_eq!(
            node.config.DKN_WALLET_PUBLIC_KEY, recovered_public_key,
            "Public key mismatch"
        );
        let address = to_address(&recovered_public_key);
        assert_eq!(address, node.address(), "Address mismatch");

        // admin node assigns the task to the compute node via Bloom Filter
        let mut bloom = FilterBuilder::new(100, 0.01).build_bloom_filter();
        bloom.add(&address);
        let filter_payload = FilterPayload::from(bloom);

        // compute node receives the filter and checks if it is tasked
        assert!(
            node.is_tasked(&filter_payload)
                .expect("Should check filter"),
            "Node should be tasked"
        );
    }
}
