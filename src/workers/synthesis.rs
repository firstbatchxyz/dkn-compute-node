use crate::{
    compute::ollama::OllamaClient,
    node::DriaComputeNode,
    utils::{
        crypto::sha256hash,
        filter::FilterPayload,
        get_current_time_nanos,
        message::{create_content_topic, WakuMessage},
    },
};
use ecies::PublicKey;
use libsecp256k1::Message;
use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};
use tokio::time;
use tokio_graceful_shutdown::errors::CancelledByShutdown;
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
    let topic: String = create_content_topic(TOPIC);
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);
    let ollama = OllamaClient::default(); // TODO: read env

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
            let mut tasks = Vec::new();
            if let Ok(messages) = node.process_topic(topic.clone()).await {
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
                let result = ollama
                    .generate(task.prompt)
                    .await
                    .expect("TODO TODO")
                    .response;

                // create h||s||e payload
                let payload = node
                    .create_payload(result, &task.public_key.as_bytes())
                    .expect("TODO TODO");
                let message = WakuMessage::new(String::from("sss"), &task.task_id, false);

                // send result to Waku network
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
