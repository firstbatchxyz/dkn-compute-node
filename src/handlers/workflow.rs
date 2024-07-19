use async_trait::async_trait;
use ollama_workflows::{Entry, Executor, ModelProvider, ProgramMemory, Workflow};
use serde::Deserialize;

use crate::errors::NodeResult;
use crate::node::DriaComputeNode;
use crate::p2p::P2PMessage;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    /// Workflow object to be parsed.
    pub(crate) workflow: Workflow,
    /// A model name (that can be parsed into `Model`) or a model provider.
    /// If model provider is given, the first matching model in the node config is used.
    pub(crate) model: String,
    /// Prompts can be provided within the workflow itself, in which case this is `None`.
    /// Otherwise, the prompt is expected to be `Some` here.
    pub(crate) prompt: Option<String>,
}

#[async_trait]
pub trait HandlesWorkflow {
    async fn handle_workflow(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()>;
}

#[async_trait]
impl HandlesWorkflow for DriaComputeNode {
    async fn handle_workflow(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()> {
        let task = self.parse_topiced_message_to_task_request::<WorkflowPayload>(message)?;

        // read model / provider from the task
        let (model_provider, model) = self.config.get_matching_model(task.input.model)?;
        log::info!("Using model {} for task {}", model, task.task_id);

        // execute workflow with cancellation
        let executor = if model_provider == ModelProvider::Ollama {
            Executor::new_at(model, &self.config.ollama.host, self.config.ollama.port)
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

        // publish the result
        match result {
            Some(result) => {
                self.send_result(result_topic, &task.public_key, &task.task_id, result)?;
            }

            // TODO: this should be error
            None => {
                log::error!("No result for task {}", task.task_id);
            }
        }

        Ok(())
    }
}
