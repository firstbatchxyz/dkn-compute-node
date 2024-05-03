use std::time::Duration;

use crate::{
    compute::{ollama::OllamaClient, payload::TaskRequestPayload},
    node::DriaComputeNode,
    utils::get_current_time_nanos,
    waku::message::WakuMessage,
};
use tokio_util::sync::CancellationToken;

/// # Synthesis Payload
///
/// A synthesis task is the task of putting a prompt to an LLM and obtaining many results, essentially growing the number of data points in a dataset,
/// hence creating synthetic data.
type SynthesisPayload = TaskRequestPayload<String>;

pub fn synthesis_worker(
    node: DriaComputeNode,
    cancellation: CancellationToken,
    topic: &'static str,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    let ollama = OllamaClient::new(None, None, None);

    tokio::spawn(async move {
        while let Err(e) = ollama.setup().await {
            log::error!("Error setting up Ollama: {}\nRetrying in 5 seconds.", e);
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => continue
            }
        }

        while let Err(e) = node.subscribe_topic(topic).await {
            log::error!(
                "Error subscribing to {}: {}\nRetrying in 5 seconds.",
                topic,
                e
            );
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => continue
            }
        }

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    if let Err(e) = node.unsubscribe_topic(topic).await {
                        log::error!("Error unsubscribing from {}: {}\nContinuing anyway.", topic, e);
                    }
                    break;
                }
                _ = tokio::time::sleep(sleep_amount) => {
                    let mut tasks = Vec::new();
                    if let Ok(messages) = node.process_topic(topic, true).await {
                        for message in messages {
                            match message.parse_payload::<SynthesisPayload>(true) {
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

                    for task in tasks {
                        // parse public key
                        let task_public_key = match hex::decode(&task.public_key) {
                            Ok(public_key) => public_key,
                            Err(e) => {
                                log::error!("Error parsing public key: {}", e);
                                continue;
                            }
                        };

                        // get prompt result from Ollama
                        let llm_result = match ollama.generate(task.input).await {
                            Ok(result) => result,
                            Err(e) => {
                                log::error!("Error generating prompt result: {}", e);
                                continue;
                            }
                        };

                        // create h||s||e payload
                        let payload = match node.create_payload(llm_result.response, &task_public_key) {
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
                        if let Err(e) = node.waku
                            .relay
                            .send_message(message)
                            .await {
                                log::error!("Error sending message: {}", e);
                                continue;
                            }
                    }
                }
            }
        }
    })
}
