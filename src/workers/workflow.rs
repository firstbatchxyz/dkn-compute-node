use async_trait::async_trait;
use ollama_workflows::{Entry, Executor, Model, ModelProvider, ProgramMemory, Workflow};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::errors::NodeResult;
use crate::node::DriaComputeNode;
use crate::p2p::P2PMessage;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    pub(crate) workflow: Workflow,
    pub(crate) model: String,
    pub(crate) prompt: Option<String>,
}

#[async_trait]
pub trait HandlesWorkflow {
    async fn handle_workflow(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()>;
}

#[async_trait]
impl HandlesWorkflow for DriaComputeNode {
    async fn handle_workflow(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()> {
        let task = self
            .parse_topiced_message_to_task_request::<WorkflowPayload>(message)
            .expect("TODO ERROR");

        // read model from the task
        let model = Model::try_from(task.input.model)?;
        let model_provider = ModelProvider::from(model.clone());
        log::info!("Using model {} for task {}", model, task.task_id);

        // execute workflow with cancellation
        let executor = if model_provider == ModelProvider::Ollama {
            // TODO: memoize this guy
            let (ollama_host, ollama_port) = get_ollama_config();
            Executor::new_at(model, &ollama_host, ollama_port)
        } else {
            Executor::new(model)
        };
        let mut memory = ProgramMemory::new();
        let entry: Option<Entry> = task
            .input
            .prompt
            .map(|prompt| Entry::try_value_or_str(&prompt));
        let result: Option<String>;
        tokio::select! {
            _ = self.cancellation.cancelled() => {
                log::info!("Received cancellation, quitting all tasks.");
                return Ok(())
            },
            exec_result = executor.execute(entry.as_ref(), task.input.workflow, &mut memory) => {
                result = Some(exec_result);
            }
        }

        match result {
            Some(result) => {
                // send result to the network
                let response =
                    P2PMessage::new_signed(result, result_topic, &self.config.secret_key);
                self.publish(response)?;
            }

            // TODO: this should be error
            None => {
                log::error!("No result for task {}", task.task_id);
            }
        }

        Ok(())
    }
}

fn get_ollama_config() -> (String, u16) {
    const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
    const DEFAULT_OLLAMA_PORT: u16 = 11434;

    let ollama_host = std::env::var("OLLAMA_HOST").unwrap_or(DEFAULT_OLLAMA_HOST.to_string());
    let ollama_port = std::env::var("OLLAMA_PORT")
        .and_then(|port_str| {
            port_str
                .parse::<u16>()
                .map_err(|_| std::env::VarError::NotPresent)
        })
        .unwrap_or(DEFAULT_OLLAMA_PORT);

    (ollama_host, ollama_port)
}
