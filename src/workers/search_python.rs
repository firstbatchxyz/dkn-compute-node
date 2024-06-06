use std::sync::Arc;
use std::time::Duration;

use crate::{
    compute::search_python::SearchPythonClient, node::DriaComputeNode, waku::message::WakuMessage,
};

/// # Search
///
/// A search task tells the agent to search an information on the Web with a set of tools provided, such
/// as web scrapers and search engine APIs.
pub fn search_worker(
    node: Arc<DriaComputeNode>,
    topic: &'static str,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    let search_client = SearchPythonClient::new();

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
                    let tasks = match node.process_topic(topic, true).await {
                        Ok(messages) => {
                            if messages.is_empty() {
                                continue;
                            }
                            log::info!("Received {} {} messages.",  messages.len(), topic);
                            node.parse_messages::<String>(messages)
                        }
                        Err(e) => {
                            log::error!("Error processing topic {}: {}", topic, e);
                            continue;
                        }
                    };

                    log::info!("Received {} {} tasks.",  tasks.len(), topic);
                    node.set_busy(true);
                    for task in tasks {
                        // parse public key
                        let task_public_key = match hex::decode(&task.public_key) {
                            Ok(public_key) => public_key,
                            Err(e) => {
                                log::error!("Error parsing public key: {}", e);
                                continue;
                            }
                        };

                        let search_result = match search_client.search(task.input).await {
                            Ok(search_result) => search_result,
                            Err(e) => {
                                log::error!("Error searching: {}", e);
                                continue;
                            }
                        };

                        // create h||s||e payload
                        let payload = match node.create_payload(search_result, &task_public_key) {
                            Ok(payload) => payload,
                            Err(e) => {
                                log::error!("Error creating payload: {}", e);
                                continue;
                            }
                        };

                        // stringify payload
                        let payload_str = match payload.to_string() {
                            Ok(payload_str) => payload_str,
                            Err(e) => {
                                log::error!("Error stringifying payload: {}", e);
                                continue;
                            }
                        };

                        // send result to Waku network
                        let message = WakuMessage::new(payload_str, &task.task_id);
                        if let Err(e) = node.send_message_once(message)
                            .await {
                                log::error!("Error sending message: {}", e);
                                continue;
                            }
                    }

                    node.set_busy(false);
                }
            }
        }
    })
}
