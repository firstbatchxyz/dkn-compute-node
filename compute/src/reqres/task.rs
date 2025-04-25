use colored::Colorize;
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::payloads::{TaskRequestPayload, TaskResponsePayload, TaskStats, TASK_RESULT_TOPIC};
use dkn_utils::DriaMessage;
use dkn_workflows::TaskBody;
use eyre::{Context, Result};

use crate::workers::task::*;
use crate::DriaComputeNode;

pub struct TaskResponder;

impl super::IsResponder for TaskResponder {
    type Request = DriaMessage; // TODO: TaskRequestPayload<TaskWorkflow>;
    type Response = DriaMessage; // TODO: TaskResponsePayload;
}

impl TaskResponder {
    /// Handles the compute message for workflows.
    pub(crate) async fn prepare_worker_input(
        compute_message: &DriaMessage,
        channel: ResponseChannel<Vec<u8>>,
    ) -> Result<(TaskWorkerInput, TaskWorkerMetadata)> {
        // parse payload
        let task = compute_message
            .parse_payload::<TaskRequestPayload<TaskBody>>()
            .wrap_err("could not parse workflow task")?;
        log::info!("Handling task {}", task.task_id);

        // record received time
        let stats = TaskStats::new().record_received_at();

        // read model / provider from the task
        let model_name = task.input.model.to_string(); // get model name, we will pass it in payload
        log::info!("Using model {} for task {}", model_name, task.task_id);

        let batchable = task.input.is_batchable();

        let task_input = TaskWorkerInput {
            body: task.input,
            task_id: task.task_id,
            row_id: task.row_id,
            stats,
            batchable,
        };

        let task_metadata = TaskWorkerMetadata {
            model_name,
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
                    "task".yellow(),
                    task_output.task_id
                );
                let payload = TaskResponsePayload::new(
                    result,
                    task_output.row_id,
                    task_output.task_id,
                    task_metadata.model_name,
                    task_output.stats.record_published_at(),
                )?;

                // convert payload to message
                let payload_str = serde_json::json!(payload).to_string();

                node.new_message(payload_str, TASK_RESULT_TOPIC)
            }
            Err(err) => {
                // use pretty display string for error logging with causes
                let err_string = format!("{:#}", err);
                log::error!("Task {} failed: {}", task_output.task_id, err_string);

                // prepare error payload
                let error_payload = TaskResponsePayload::new_error(
                    err_string,
                    task_output.row_id,
                    task_output.task_id,
                    task_metadata.model_name,
                    task_output.stats.record_published_at(),
                );
                let error_payload_str = serde_json::json!(error_payload).to_string();

                node.new_message(error_payload_str, TASK_RESULT_TOPIC)
            }
        };

        // respond through the channel
        node.p2p
            .respond(response.into(), task_metadata.channel)
            .await?;

        Ok(())
    }
}
