use crate::{
    compute::ollama::OllamaClient,
    node::DriaComputeNode,
    utils::{filter::FilterPayload, get_current_time_nanos},
    waku::message::WakuMessage,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

const TOPIC: &str = "synthesis";
const SLEEP_MILLIS: u64 = 500;

/// # Synthesis Payload
///
/// A synthesis task is the task of putting a prompt to an LLM and obtaining many results, essentially growing the number of data points in a dataset,
/// hence creating synthetic data.
///
/// ## Fields
///
/// - `task_id`: The unique identifier of the task.
/// - `deadline`: The deadline of the task in nanoseconds.
/// - `prompt`: The prompt to be given to the LLM.
/// - `filter`: The filter of the task.
/// - `public_key`: The public key of the requester.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct SynthesisPayload {
    task_id: String,
    deadline: u128,
    prompt: String,
    filter: FilterPayload,
    public_key: String,
}

pub fn synthesis_worker(
    node: DriaComputeNode,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);
    let ollama = OllamaClient::new(None, None, None);

    tokio::spawn(async move {
        ollama.setup().await.expect("TODO TODO");

        match node.subscribe_topic(TOPIC).await {
            Ok(_) => {
                log::info!("Subscribed to {}", TOPIC);
            }
            Err(e) => {
                log::error!("Error subscribing to {}", e);
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
                    let mut tasks = Vec::new();
                    if let Ok(messages) = node.process_topic(TOPIC, true).await {

                        for message in messages {
                            let task = message
                                .parse_payload::<SynthesisPayload>(true)
                                .expect("TODO TODO");

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
                        }
                    }

                    for task in tasks {
                        // get prompt result from Ollama
                        let llm_result = ollama.generate(task.prompt).await.expect("TODO TODO");

                        // create h||s||e payload
                        let payload = node
                            .create_payload(llm_result.response, task.public_key.as_bytes())
                            .expect("TODO TODO");
                        let message = WakuMessage::new(String::from(payload), &task.task_id);

                        // send result to Waku network
                        node.waku
                            .relay
                            .send_message(message)
                            .await
                            .expect("TODO TODO");
                    }
                }
            }
        }
    })
}
