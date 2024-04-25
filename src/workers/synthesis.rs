use crate::{
    compute::ollama::OllamaClient,
    node::DriaComputeNode,
    utils::{crypto::sha256hash, filter::FilterPayload, get_current_time_nanos},
    waku::message::WakuMessage,
};
use ecies::PublicKey;
use libsecp256k1::Message;
use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use tokio::time;
use tokio_util::sync::CancellationToken;

const TOPIC: &str = "synthesis";
const SLEEP_MILLIS: u64 = 500;

/// # Synthesis Payload
///
/// A synthesis task is the task of putting a prompt to an LLM and obtaining many results, essentially growing the number of data points in a dataset,
/// hence creating synthetic data.
///
/// ## Fields
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
    mut node: DriaComputeNode,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);
    let ollama = OllamaClient::default(); // TODO: read env

    tokio::spawn(async move {
        match node.subscribe_topic(TOPIC).await {
            Ok(_) => {
                println!("Subscribed to {}", TOPIC);
            }
            Err(e) => {
                println!("Error subscribing to {}", e);
            }
        }

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => { break; }
                _ = tokio::time::sleep(sleep_amount) => {
                    let mut tasks = Vec::new();
                    if let Ok(messages) = node.process_topic(TOPIC).await {
                        println!("Synthesis tasks: {:?}", messages);

                        for message in messages {
                            let task = message
                                .parse_payload::<SynthesisPayload>()
                                .expect("TODO TODO"); // TODO: error handling

                            // check deadline
                            if get_current_time_nanos() >= task.deadline.clone() {
                                continue;
                            }

                            // check task inclusion
                            if !node.is_tasked(task.filter.clone()) {
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
                            .create_payload(llm_result.response, &task.public_key.as_bytes())
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
