use async_trait::async_trait;
use eyre::{eyre, Result};
use libp2p::gossipsub::MessageAcceptance;
use ollama_workflows::{Entry, Executor, ModelProvider, ProgramMemory, Workflow};
use serde::Deserialize;

use crate::node::DriaComputeNode;
use crate::p2p::P2PMessage;
use crate::utils::get_current_time_nanos;
use crate::utils::payload::{TaskRequest, TaskRequestPayload};

use super::ComputeHandler;

pub struct WorkflowHandler;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    /// Workflow object to be parsed.
    pub(crate) workflow: Workflow,
    /// A lıst of model (that can be parsed into `Model`) or model provider names.
    /// If model provider is given, the first matching model in the node config is used for that.
    /// From the given list, a random choice will be made for the task.
    pub(crate) model: Vec<String>,
    /// Prompts can be provided within the workflow itself, in which case this is `None`.
    /// Otherwise, the prompt is expected to be `Some` here.
    pub(crate) prompt: Option<String>,
}

#[async_trait]
impl ComputeHandler for WorkflowHandler {
    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: P2PMessage,
        result_topic: &str,
    ) -> Result<MessageAcceptance> {
        let task = message.parse_payload::<TaskRequestPayload<WorkflowPayload>>(true)?;

        // check if deadline is past or not
        let current_time = get_current_time_nanos();
        if current_time >= task.deadline {
            log::debug!(
                "Task (id: {}) is past the deadline, ignoring. (local: {}, deadline: {})",
                task.task_id,
                current_time,
                task.deadline
            );

            // ignore the message
            return Ok(MessageAcceptance::Ignore);
        }

        // check task inclusion via the bloom filter
        if !task.filter.contains(&node.config.address)? {
            log::info!(
                "Task {} does not include this node within the filter.",
                task.task_id
            );

            // accept the message, someonelse may be included in filter
            return Ok(MessageAcceptance::Accept);
        }

        // obtain public key from the payload
        let task_public_key = hex::decode(&task.public_key)?;

        let task = TaskRequest {
            task_id: task.task_id,
            input: task.input,
            public_key: task_public_key,
        };

        // read model / provider from the task
        let (model_provider, model) = node
            .config
            .model_config
            .get_any_matching_model(task.input.model)?;
        log::info!("Using model {} for task {}", model, task.task_id);

        // prepare workflow executor
        let executor = if model_provider == ModelProvider::Ollama {
            Executor::new_at(
                model,
                &node.config.ollama_config.host,
                node.config.ollama_config.port,
            )
        } else {
            Executor::new(model)
        };
        let mut memory = ProgramMemory::new();
        let entry: Option<Entry> = task
            .input
            .prompt
            .map(|prompt| Entry::try_value_or_str(&prompt));

        // execute workflow with cancellation
        let result: String;
        tokio::select! {
            _ = node.cancellation.cancelled() => {
                log::info!("Received cancellation, quitting all tasks.");
                return Ok(MessageAcceptance::Accept)
            },
            exec_result = executor.execute(entry.as_ref(), task.input.workflow, &mut memory) => {
                match exec_result {
                    Ok(exec_result) => {
                        result = exec_result;
                    }
                    Err(e) => {
                        return Err(eyre!("Workflow failed with error {}", e));
                    }
                }
            }
        }

        // publish the result
        node.send_result(result_topic, &task.public_key, &task.task_id, result)?;
        Ok(MessageAcceptance::Accept)
    }
}
