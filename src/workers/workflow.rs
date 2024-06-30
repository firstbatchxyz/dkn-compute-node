use ollama_workflows::{Entry, Executor, Model, ProgramMemory, Workflow};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::node::DriaComputeNode;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    pub(crate) workflow: Workflow,
    pub(crate) model: String,
    pub(crate) prompt: Option<String>,
}

const REQUEST_TOPIC: &str = "workflow";
const RESPONSE_TOPIC: &str = "results";

pub fn workflow_worker(
    node: Arc<DriaComputeNode>,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        node.subscribe_topic(REQUEST_TOPIC).await;
        node.subscribe_topic(RESPONSE_TOPIC).await;

        loop {
            tokio::select! {
                _ = node.cancellation.cancelled() => break,
                _ = tokio::time::sleep(sleep_amount) => {
                    let tasks = match node.process_topic(REQUEST_TOPIC, true).await {
                        Ok(messages) => {
                            if messages.is_empty() {
                                continue;
                            }
                            node.parse_messages::<WorkflowPayload>(messages, true)
                        }
                        Err(e) => {
                            log::error!("Error processing topic {}: {}", REQUEST_TOPIC, e);
                            continue;
                        }
                    };
                    if tasks.is_empty() {
                        log::info!("No {} tasks.", REQUEST_TOPIC);
                    } else {
                        node.set_busy(true);

                        log::info!("Processing {} {} tasks.", tasks.len(), REQUEST_TOPIC);
                        for task in &tasks {
                            log::debug!("Task ID: {}", task.task_id);
                        }

                        for task in tasks {
                            // read model from the task
                            let model = match Model::try_from(task.input.model) {
                                Ok(model) => model,
                                Err(e) => {
                                    log::error!("Could not read model: {}\nSkipping task {}", e, task.task_id);
                                    continue;
                                }
                            };
                            log::info!("Using model {} for task {}", model, task.task_id);

                            // execute workflow with cancellation
                            let executor = Executor::new(model);
                            let mut memory = ProgramMemory::new();
                            let entry: Option<Entry> = task.input.prompt.map(|prompt| Entry::try_value_or_str(&prompt));
                            let result: Option<String>;
                            tokio::select! {
                                _ = node.cancellation.cancelled() => {
                                    log::info!("Received cancellation, quitting all tasks.");
                                    break;
                                },
                                exec_result = executor.execute(entry.as_ref(), task.input.workflow, &mut memory) => {
                                    result = Some(exec_result);
                                }
                            }

                            match result {
                                Some(result) => {
                                    // send result to the network
                                    if let Err(e) = node.send_result(RESPONSE_TOPIC, &task.public_key, &task.task_id, result).await {
                                        log::error!("Error sending task result: {}", e);
                                        continue;
                                    };
                                }
                                None => {
                                    log::error!("No result for task {}", task.task_id);
                                    continue;
                                }
                            }

                        }

                        node.set_busy(false);
                    }
                }
            }
        }

        node.unsubscribe_topic_ignored(REQUEST_TOPIC).await;
        node.unsubscribe_topic_ignored(RESPONSE_TOPIC).await;
    })
}
