use std::time::Duration;
use std::sync::Arc;
use serde_json::json;

use crate::{
    compute::payload::TaskRequestPayload,
    node::DriaComputeNode,
    utils::get_current_time_nanos,
    utils::http::BaseClient,
    waku::message::WakuMessage,
    config::DriaComputeNodeConfig,
};


/// # Search Payload
///
/// A synthesis task is the task of putting a prompt to an LLM and obtaining many results, essentially growing the number of data points in a dataset,
/// hence creating synthetic data.
type SearchPayload = TaskRequestPayload<String>;

pub fn search_worker(
    node: Arc<DriaComputeNode>,
    topic: &'static str,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {

    let config = node.config.clone();
    let http_client = BaseClient::new(config.SEARCH_AGENT_URL.to_string());

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
                    let mut tasks = Vec::new();
                    if let Ok(messages) = node.process_topic(topic, true).await {
                        if messages.is_empty() {
                            continue;
                        }
                        log::info!("Received {} search-python tasks.", messages.len());

                        for message in messages {
                            match message.parse_payload::<SearchPayload>(true) {
                                Ok(task) => {
                                    // check deadline
                                    if get_current_time_nanos() >= task.deadline {
                                        log::debug!("{}", format!("Skipping {} due to deadline.", task.task_id));
                                        continue;
                                    }

                                    // check task inclusion
                                    match node.is_tasked(&task.filter) {
                                        Ok(is_tasked) => {
                                            if is_tasked {
                                                log::debug!("{}", format!("Skipping {} due to filter.", task.task_id));
                                                continue;
                                            }
                                        },
                                        Err(e) => {
                                            log::error!("Error checking task inclusion: {}", e);
                                            continue;
                                        }
                                    }

                                    tasks.push(task);
                                },
                                Err(e) => {
                                    log::error!("Error parsing payload: {}", e);
                                    continue;
                                }
                            }
                        }
                    }
                    // Set node to busy
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

                        let body = json!({
                            "query": task.input,
                            "with_manager": config.SEARCH_AGENT_MANAGER,
                        });                    
                        let r = match http_client.post("search", body).await{
                            Ok(response) => response,
                            Err(e) => {
                                eprintln!("Error sending search query to search-agent-python: {:?}", e);
                                continue;
                            }
                        };

                        let search_result = match r.text().await{
                            Ok(response) => response,
                            Err(e) => {
                                eprintln!("Error parsing search-agent-python response: {:?}", e);
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

                    // Set node to not busy
                    node.set_busy(false);
                }
            }
        }
    })
}
