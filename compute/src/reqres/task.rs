#![allow(unused)]

use colored::Colorize;
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_workflows::{Entry, Executor, ModelProvider, Workflow};
use eyre::{eyre, Context, Result};
use libsecp256k1::PublicKey;
use serde::Deserialize;

use crate::payloads::*;
use crate::utils::DriaMessage;
use crate::workers::task::*;
use crate::DriaComputeNode;

use super::IsResponder;

pub struct TaskResponder;

impl IsResponder for TaskResponder {
    type Request = DriaMessage; // TODO: TaskRequestPayload<WorkflowPayload>;
    type Response = DriaMessage; // TODO: TaskResponsePayload;
}

#[derive(Debug, Deserialize)]
pub struct TaskPayload {
    /// [Workflow](https://github.com/andthattoo/ollama-workflows/blob/main/src/program/workflow.rs) object to be parsed.
    pub(crate) workflow: Workflow,
    /// A lıst of model (that can be parsed into `Model`) or model provider names.
    /// If model provider is given, the first matching model in the node config is used for that.
    /// From the given list, a random choice will be made for the task.
    pub(crate) model: Vec<String>,
    /// Prompts can be provided within the workflow itself, in which case this is `None`.
    /// Otherwise, the prompt is expected to be `Some` here.
    pub(crate) prompt: Option<String>,
}

impl TaskResponder {
    /// Handles the compute message for workflows.
    pub(crate) async fn prepare_worker_input(
        node: &mut DriaComputeNode,
        compute_message: &DriaMessage,
        channel: ResponseChannel<Vec<u8>>,
    ) -> Result<(TaskWorkerInput, TaskWorkerMetadata)> {
        // parse payload
        let task = compute_message
            .parse_payload::<TaskRequestPayload<TaskPayload>>()
            .wrap_err("could not parse workflow task")?;
        log::info!("Handling task {}", task.task_id);

        let stats = TaskStats::new().record_received_at();

        // check if deadline is past or not
        // with request-response, we dont expect this to happen much
        if chrono::Utc::now() >= task.deadline {
            return Err(eyre!(
                "Task {} is past the deadline, ignoring",
                task.task_id
            ));
        }

        // obtain public key from the payload
        // do this early to avoid unnecessary processing
        let task_public_key_bytes =
            hex::decode(&task.public_key).wrap_err("could not decode public key")?;
        let task_public_key = PublicKey::parse_slice(&task_public_key_bytes, None)?;

        // read model / provider from the task
        let (model_provider, model) = node
            .config
            .workflows
            .get_any_matching_model(task.input.model)?;
        let model_name = model.to_string(); // get model name, we will pass it in payload
        log::info!("Using model {} for task {}", model_name, task.task_id);

        // prepare workflow executor
        let (executor, batchable) = if model_provider == ModelProvider::Ollama {
            (
                Executor::new_at(
                    model,
                    &node.config.workflows.ollama.host,
                    node.config.workflows.ollama.port,
                ),
                false,
            )
        } else {
            (Executor::new(model), true)
        };

        // prepare entry from prompt
        let entry: Option<Entry> = task
            .input
            .prompt
            .map(|prompt| Entry::try_value_or_str(&prompt));

        // get workflow as well
        let workflow = task.input.workflow;

        let task_input = TaskWorkerInput {
            entry,
            executor,
            workflow,
            task_id: task.task_id,
            stats,
            batchable,
        };

        let task_metadata = TaskWorkerMetadata {
            model_name,
            public_key: task_public_key,
            channel,
        };

        Ok((task_input, task_metadata))
    }

    /// Handles the result of a workflow task.
    pub(crate) async fn send_output(
        node: &mut DriaComputeNode,
        task_output: TaskWorkerOutput,
        task_metadata: TaskWorkerMetadata,
    ) -> Result<()> {
        let response = match task_output.result {
            Ok(result) => {
                // prepare signed and encrypted payload
                log::info!(
                    "Publishing {} result for {}",
                    "task".green(),
                    task_output.task_id
                );
                let payload = TaskResponsePayload::new(
                    result,
                    &task_output.task_id,
                    &task_metadata.public_key,
                    task_metadata.model_name,
                    task_output.stats.record_published_at(),
                )?;

                // convert payload to message
                let payload_str = serde_json::json!(payload).to_string();

                node.new_message(payload_str, "response")
            }
            Err(err) => {
                // use pretty display string for error logging with causes
                let err_string = format!("{:#}", err);
                log::error!("Task {} failed: {}", task_output.task_id, err_string);

                // prepare error payload
                let error_payload = TaskErrorPayload {
                    task_id: task_output.task_id,
                    error: err_string,
                    model: task_metadata.model_name,
                    stats: task_output.stats.record_published_at(),
                };
                let error_payload_str = serde_json::json!(error_payload).to_string();

                node.new_message(error_payload_str, "response")
            }
        };

        // respond through the channel
        let data = response.to_bytes()?;
        node.p2p.respond(data, task_metadata.channel).await?;

        Ok(())
    }
}
