use crate::{
    compute::{ollama::OllamaClient, payload::TaskRequestPayload},
    node::DriaComputeNode,
    utils::get_current_time_nanos,
    waku::message::WakuMessage,
};
use tokio_util::sync::CancellationToken;

const TOPIC: &str = "synthesis";
const SLEEP_MILLIS: u64 = 500;

/// # Synthesis Payload
///
/// A synthesis task is the task of putting a prompt to an LLM and obtaining many results, essentially growing the number of data points in a dataset,
/// hence creating synthetic data.
type SynthesisPayload = TaskRequestPayload<String>;

pub fn synthesis_worker(
    node: DriaComputeNode,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);
    let ollama = OllamaClient::new(None, None, None);

    tokio::spawn(async move {
        while let Err(e) = ollama.setup().await {
            log::error!("Error setting up Ollama: {}\nRetrying in 5 seconds.", e);
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => continue
            }
        }

        while let Err(e) = node.subscribe_topic(TOPIC).await {
            log::error!(
                "Error subscribing to {}: {}\nRetrying in 5 seconds.",
                TOPIC,
                e
            );
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => continue
            }
        }
        log::info!("Subscribed to {}", TOPIC);

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    if let Err(e) = node.unsubscribe_topic(TOPIC).await {
                        log::error!("Error unsubscribing from {}: {}\nContinuing anyway.", TOPIC, e);
                    }
                    break;
                }
                _ = tokio::time::sleep(sleep_amount) => {
                    let mut tasks = Vec::new();
                    if let Ok(messages) = node.process_topic(TOPIC, true).await {
                        for message in messages {
                            match message.parse_payload::<SynthesisPayload>(true) {
                                Ok(task) => {
                                    // check deadline
                                    if get_current_time_nanos() >= task.deadline {
                                        log::debug!("{}", format!("Skipping {} due to deadline.", task.task_id));
                                        continue;
                                    }

                                    // check task inclusion
                                    if !node.is_tasked(task.filter.clone()) {
                                        log::debug!("{}", format!("Skipping {} due to filter.", task.task_id));
                                        continue;
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
                        // get prompt result from Ollama
                        let llm_result = match ollama.generate(task.input).await {
                            Ok(result) => result,
                            Err(e) => {
                                log::error!("Error generating prompt result: {}", e);
                                continue;
                            }
                        };

                        // create h||s||e payload
                        let payload = match node.create_payload(llm_result.response, task.public_key.as_bytes()) {
                            Ok(payload) => payload,
                            Err(e) => {
                                log::error!("Error creating payload: {}", e);
                                continue;
                            }
                        };

                        // send result to Waku network
                        let message = WakuMessage::new(String::from(payload), &task.task_id);
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
