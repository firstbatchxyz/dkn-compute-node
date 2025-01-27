#![allow(unused)]

use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::get_current_time_nanos;
use dkn_workflows::{Entry, Executor, ModelProvider, Workflow};
use eyre::{eyre, Context, Result};
use libsecp256k1::PublicKey;
use serde::Deserialize;

use crate::payloads::*;
use crate::utils::DriaMessage;
use crate::workers::workflow::*;
use crate::DriaComputeNode;

use super::IsResponder;

pub struct WorkflowResponder;

impl IsResponder for WorkflowResponder {
    type Request = DriaMessage; // TaskRequestPayload<WorkflowPayload>;
    type Response = DriaMessage; // TaskResponsePayload;
}

#[derive(Debug, Deserialize)]
pub struct WorkflowPayload {
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

impl WorkflowResponder {
    /// Handles the compute message for workflows.
    ///
    /// - FIXME: DOES NOT CHECK FOR FILTER AS IT IS NO LONGER USED
    /// - FIXME: GIVES ERROR ON DEADLINE PAST CASE, BUT WE DONT NEED DEADLINE AS WELL
    pub(crate) async fn handle_compute(
        node: &mut DriaComputeNode,
        compute_message: &DriaMessage,
    ) -> Result<WorkflowsWorkerInput> {
        // parse payload
        let task = compute_message
            .parse_payload::<TaskRequestPayload<WorkflowPayload>>()
            .wrap_err("could not parse workflow task")?;
        log::info!("Handling task {}", task.task_id);

        let stats = TaskStats::new().record_received_at();

        // check if deadline is past or not
        // with request-response, we dont expect this to happen much
        if get_current_time_nanos() >= task.deadline {
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

        Ok(WorkflowsWorkerInput {
            entry,
            executor,
            workflow,
            model_name,
            task_id: task.task_id,
            public_key: task_public_key,
            stats,
            batchable,
        })
    }

    /// Handles the result of a workflow task.
    pub(crate) async fn handle_respond(
        node: &mut DriaComputeNode,
        task: WorkflowsWorkerOutput,
    ) -> Result<()> {
        let response = match task.result {
            Ok(result) => {
                // prepare signed and encrypted payload
                log::info!("Publishing result for task {}", task.task_id);
                let payload = TaskResponsePayload::new(
                    result,
                    &task.task_id,
                    &task.public_key,
                    task.model_name,
                    task.stats.record_published_at(),
                )?;

                // convert payload to message
                let payload_str = serde_json::json!(payload).to_string();

                node.new_message(payload_str, "response")
            }
            Err(err) => {
                // use pretty display string for error logging with causes
                let err_string = format!("{:#}", err);
                log::error!("Task {} failed: {}", task.task_id, err_string);

                // prepare error payload
                let error_payload = TaskErrorPayload {
                    task_id: task.task_id.clone(),
                    error: err_string,
                    model: task.model_name,
                    stats: task.stats.record_published_at(),
                };
                let error_payload_str = serde_json::json!(error_payload).to_string();

                node.new_message(error_payload_str, "response")
            }
        };

        // respond through the channel
        todo!("TODO: respond through the channel");

        Ok(())
    }
}
