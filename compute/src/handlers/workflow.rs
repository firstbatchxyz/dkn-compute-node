use std::time::Instant;

use async_trait::async_trait;
use dkn_p2p::libp2p::gossipsub::MessageAcceptance;
use dkn_workflows::{Entry, Executor, ModelProvider, ProgramMemory, Workflow};
use eyre::{eyre, Context, Result};
use libsecp256k1::PublicKey;
use serde::Deserialize;

use crate::payloads::{TaskErrorPayload, TaskRequestPayload, TaskResponsePayload, TaskStats};
use crate::utils::{get_current_time_nanos, DKNMessage};
use crate::DriaComputeNode;

use super::ComputeHandler;

pub struct WorkflowHandler;

#[derive(Debug, Deserialize)]
struct WorkflowPayload {
    /// [Workflow](https://github.com/andthattoo/ollama-workflows/) object to be parsed.
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
    const LISTEN_TOPIC: &'static str = "task";
    const RESPONSE_TOPIC: &'static str = "results";

    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: DKNMessage,
    ) -> Result<MessageAcceptance> {
        let task = message
            .parse_payload::<TaskRequestPayload<WorkflowPayload>>(true)
            .wrap_err("Could not parse workflow task")?;
        let mut task_stats = TaskStats::default().record_received_at();

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

            // accept the message, someone else may be included in filter
            return Ok(MessageAcceptance::Accept);
        }

        // read model / provider from the task
        let (model_provider, model) = node
            .config
            .workflows
            .get_any_matching_model(task.input.model)?;
        let model_name = model.to_string(); // get model name, we will pass it in payload
        log::info!("Using model {} for task {}", model_name, task.task_id);

        // prepare workflow executor
        let executor = if model_provider == ModelProvider::Ollama {
            Executor::new_at(
                model,
                &node.config.workflows.ollama.host,
                node.config.workflows.ollama.port,
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
        let exec_result: Result<String>;
        let exec_started_at = Instant::now();
        tokio::select! {
            _ = node.cancellation.cancelled() => {
                log::info!("Received cancellation, quitting all tasks.");
                return Ok(MessageAcceptance::Accept);
            },
            exec_result_inner = executor.execute(entry.as_ref(), &task.input.workflow, &mut memory) => {
                exec_result = exec_result_inner.map_err(|e| eyre!("Execution error: {}", e.to_string()));
            }
        }
        task_stats = task_stats.record_execution_time(exec_started_at);

        let (message, acceptance) = match exec_result {
            Ok(result) => {
                // obtain public key from the payload
                let task_public_key_bytes =
                    hex::decode(&task.public_key).wrap_err("Could not decode public key")?;
                let task_public_key = PublicKey::parse_slice(&task_public_key_bytes, None)?;

                // prepare signed and encrypted payload
                let payload = TaskResponsePayload::new(
                    result,
                    &task.task_id,
                    &task_public_key,
                    &node.config.secret_key,
                    model_name,
                    task_stats.record_published_at(),
                )?;
                let payload_str = serde_json::to_string(&payload)
                    .wrap_err("Could not serialize response payload")?;

                // prepare signed message
                log::debug!(
                    "Publishing result for task {}\n{}",
                    task.task_id,
                    payload_str
                );
                let message = DKNMessage::new(payload_str, Self::RESPONSE_TOPIC);
                // accept so that if there are others included in filter they can do the task
                (message, MessageAcceptance::Accept)
            }
            Err(err) => {
                // use pretty display string for error logging with causes
                let err_string = format!("{:#}", err);
                log::error!("Task {} failed: {}", task.task_id, err_string);

                // prepare error payload
                let error_payload = TaskErrorPayload {
                    task_id: task.task_id.clone(),
                    error: err_string,
                    model: model_name,
                    stats: task_stats.record_published_at(),
                };
                let error_payload_str = serde_json::to_string(&error_payload)
                    .wrap_err("Could not serialize error payload")?;

                // prepare signed message
                let message = DKNMessage::new_signed(
                    error_payload_str,
                    Self::RESPONSE_TOPIC,
                    &node.config.secret_key,
                );
                // ignore just in case, workflow may be bugged
                (message, MessageAcceptance::Ignore)
            }
        };

        // try publishing the result
        if let Err(publish_err) = node.publish(message).await {
            let err_msg = format!("Could not publish result: {:?}", publish_err);
            log::error!("{}", err_msg);

            let payload = serde_json::json!({
                "taskId": task.task_id,
                "error": err_msg,
            });
            let message = DKNMessage::new_signed(
                payload.to_string(),
                Self::RESPONSE_TOPIC,
                &node.config.secret_key,
            );
            node.publish(message).await?;
        }

        Ok(acceptance)
    }
}
