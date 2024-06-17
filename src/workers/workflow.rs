use ollama_workflows::{Entry, Executor, Model, ProgramMemory, Workflow};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::node::DriaComputeNode;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    pub(crate) workflow: Workflow,
    pub(crate) model: String,
    pub(crate) prompt: String,
}

pub fn workflow_worker(
    node: Arc<DriaComputeNode>,
    topic: &'static str,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    // this ID is given in the workflow itself, but within Dria we always
    // use "final_result" for this ID.
    let final_result_id = "final_result".to_string();

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
                            node.parse_messages::<WorkflowPayload>(messages, true)
                        }
                        Err(e) => {
                            log::error!("Error processing topic {}: {}", topic, e);
                            continue;
                        }
                    };
                    if tasks.is_empty() {
                        log::info!("No {} tasks.", topic);
                    } else {
                        node.set_busy(true);
                        log::info!("Processing {} {} tasks.", tasks.len(), topic);
                        for task in &tasks {
                            log::debug!("Task ID: {}", task.task_id);
                        }

                        for task in tasks {
                            // read model from the task
                            let model = Model::try_from(task.input.model.clone()).unwrap_or_else(|model| {
                                log::error!("Invalid model provided: {}, defaulting.", model);
                                Model::default()
                            });
                            log::info!("Using model {}", model);

                            // execute workflow
                            let executor = Executor::new(model);
                            let mut memory = ProgramMemory::new();
                            let entry = Entry::String(task.input.prompt);
                            executor.execute(Some(&entry), task.input.workflow, &mut memory).await;

                            // read final result from memory
                            let result = match memory.read(&final_result_id) {
                                Some(entry) => entry.to_string(),
                                None => {
                                    log::error!("No final result found in memory for task {}", task.task_id);
                                    continue;
                                },
                            };

                            if let Err(e) = node.send_task_result(&task.task_id, &task.public_key, result).await {
                                log::error!("Error sending task result: {}", e);
                            };
                        }

                        node.set_busy(false);
                    }
                }
            }
        }
    })
}
