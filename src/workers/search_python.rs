use std::sync::Arc;
use std::time::Duration;

use crate::{compute::search_python::SearchPythonClient, node::DriaComputeNode};

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

                    node.set_busy(true);
                    log::info!("Received {} {} tasks.",  tasks.len(), topic);
                    for task in tasks {
                        let result = match search_client.search(task.input).await {
                            Ok(result) => result,
                            Err(e) => {
                                log::error!("Error searching: {}", e);
                                continue;
                            }
                        };

                        if let Err(e) = node.send_task_result(&task.task_id, &task.public_key, result).await {
                            log::error!("Error sending task result: {}", e);
                        };
                    }

                    node.set_busy(false);
                }
            }
        }
    })
}
