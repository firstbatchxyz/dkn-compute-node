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
    // this ID is given in the workflow itself, but within Dria we always
    // use "final_result" for this ID.
    let final_result_id = "final_result".to_string();

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
                            let entry: Option<Entry> = task.input.prompt.map(|prompt| Entry::String(prompt));
                            tokio::select! {
                                _ = node.cancellation.cancelled() => {
                                    log::info!("Received cancellation, quitting all tasks.");
                                    break;
                                },
                                _ = executor.execute(entry.as_ref(), task.input.workflow, &mut memory) => ()
                            }

                            // read final result from memory
                            // let result = match memory.read(&final_result_id) {
                            //     Some(entry) => entry.to_string(),
                            //     None => {
                            //         log::error!("No final result found in memory for task {}", task.task_id);
                            //         continue;
                            //     },
                            // };
                            // TODO: temporary for fix, for Workflow 1 (w1)
                            let res: Option<Vec<Entry>> = memory.get_all(&"history".to_string());
                            let mut vars_all: Vec<String> = vec![];
                            if let Some(res) = res {
                                for entry in res {
                                    let sstr = entry.to_string();
                                    let vars: Vec<String> = sstr.split("\n").map(|s| s.to_string()).collect();
                                    vars_all.extend(vars);
                                }
                            }
                            let result = serde_json::to_string(&vars_all).unwrap();

                            // send result to the response
                            if let Err(e) = node.send_result(RESPONSE_TOPIC, &task.public_key, &task.task_id, result).await {
                                log::error!("Error sending task result: {}", e);
                                continue;
                            };
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
