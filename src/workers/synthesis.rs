use std::sync::Arc;
use std::time::Duration;

use crate::{
    compute::{
        llm::common::{create_llm, ModelProvider},
        payload::TaskRequestPayload,
    },
    config::constants::*,
    node::DriaComputeNode,
    utils::get_current_time_nanos,
    waku::message::WakuMessage,
};

type SynthesisPayload = TaskRequestPayload<String>;

/// # Synthesis
///
/// A synthesis task is the task of putting a prompt to an LLM and obtaining many results, essentially growing the number of data points in a dataset,
/// hence creating synthetic data.
pub fn synthesis_worker(
    node: Arc<DriaComputeNode>,
    topic: &'static str,
    sleep_amount: Duration,
    model_provider: Option<String>,
    model_name: Option<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (model_provider, model_name) = parse_model_info(model_provider, model_name);
        log::info!("Using {} with {}", model_provider, model_name);

        let llm = match create_llm(model_provider, model_name, node.cancellation.clone()).await {
            Ok(llm) => llm,
            Err(e) => {
                log::error!("Could not create LLM: {}, exiting worker.", e);
                return;
            }
        };

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
                        log::info!("Received {} synthesis tasks.", messages.len());

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

                    // TODO: wait for busy lock
                    node.set_busy(true);

                    // sort tasks by deadline, closer deadline processed first
                    tasks.sort_by(|a, b| a.deadline.cmp(&b.deadline));
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
                        let llm_result = match llm.invoke(&task.input).await {
                            Ok(result) => result,
                            Err(e) => {
                                log::error!("Error generating prompt result: {}", e);
                                continue;
                            }
                        };

                        // create h||s||e payload
                        let payload = match node.create_payload(llm_result, &task_public_key) {
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

pub fn parse_model_info(
    model_provider: Option<String>,
    model_name: Option<String>,
) -> (ModelProvider, String) {
    let model_provider: ModelProvider = model_provider
        .unwrap_or(DEFAULT_DKN_SYNTHESIS_MODEL_PROVIDER.to_string())
        .into();

    let model_name = model_name.unwrap_or_else(|| {
        match &model_provider {
            ModelProvider::OpenAI => DEFAULT_DKN_SYNTHESIS_MODEL_NAME_OPENAI.to_string(),
            ModelProvider::Ollama => DEFAULT_DKN_SYNTHESIS_MODEL_NAME_OLLAMA.to_string(),
        }
        .to_string()
    });

    (model_provider, model_name)
}
