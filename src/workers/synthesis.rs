use std::sync::Arc;
use std::time::Duration;

use crate::{
    compute::llm::common::{create_llm, ModelProvider},
    config::constants::*,
    node::DriaComputeNode,
};

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
                    let tasks = match node.process_topic(topic, true).await {
                        Ok(messages) => {
                            if messages.is_empty() {
                                continue;
                            }
                            node.parse_messages::<String>(messages, true)
                        }
                        Err(e) => {
                            log::error!("Error processing topic {}: {}", topic, e);
                            continue;
                        }
                    };

                    node.set_busy(true);
                    log::info!("Processing {} {} tasks.", tasks.len(), topic);
                    for task in &tasks {
                        log::debug!("Task ID: {}", task.task_id);
                    }

                    for task in tasks {
                        let llm_result = match llm.invoke(&task.input).await {
                            Ok(result) => result,
                            Err(e) => {
                                log::error!("Error generating prompt result: {}", e);
                                continue;
                            }
                        };

                        if let Err(e) = node.send_task_result(&task.task_id, &task.public_key, llm_result).await {
                            log::error!("Error sending task result: {}", e);
                        };
                    }

                    node.set_busy(false);
                }
            }
        }
    })
}

/// Given a model provier option, and a model name option, return the model provider and model name.
///
/// - If model provider is `None`, it will default.
/// - If model name is `None`, it will default to some model name with respect ot the model provider.
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
