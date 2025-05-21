use colored::Colorize;
use dkn_executor::TaskBody;
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::payloads::{TaskRequestPayload, TaskResponsePayload, TaskStats, TASK_RESULT_TOPIC};
use dkn_utils::DriaMessage;
use eyre::{Context, Result};

use crate::workers::task::*;
use crate::DriaComputeNode;

pub struct TaskResponder;

impl super::IsResponder for TaskResponder {
    type Request = DriaMessage; // TODO: can we do this typed?
    type Response = DriaMessage; // TODO: can we do this typed?
}

impl TaskResponder {
    pub(crate) async fn parse_task_request(
        node: &mut DriaComputeNode,
        compute_message: &DriaMessage,
        channel: ResponseChannel<Vec<u8>>,
    ) -> Result<(TaskWorkerInput, TaskWorkerMetadata)> {
        let task = compute_message
            .parse_payload::<TaskRequestPayload<TaskBody>>()
            .wrap_err("could not parse task payload")?;
        let stats = TaskStats::new().record_received_at();
        log::info!(
            "Handling {} {} with model {}",
            "task".yellow(),
            task.row_id,
            task.input.model.to_string().yellow()
        );

        // check if the model is available in this node, if so
        // it will return an executor that can run this model
        let executor = node
            .config
            .executors
            .get_executor(&task.input.model)
            .await
            .wrap_err("could not get an executor")?;

        let task_metadata = TaskWorkerMetadata {
            task_id: task.task_id,
            file_id: task.file_id,
            model_name: task.input.model.to_string(),
            channel,
        };
        let task_input = TaskWorkerInput {
            executor,
            task: task.input,
            row_id: task.row_id,
            stats,
        };

        Ok((task_input, task_metadata))
    }

    /// Handles the result of a task.
    pub(crate) async fn send_task_output(
        node: &mut DriaComputeNode,
        task_output: TaskWorkerOutput,
        task_metadata: TaskWorkerMetadata,
    ) -> Result<()> {
        let response = match task_output.result {
            Ok(result) => {
                // prepare signed and encrypted payload
                log::info!(
                    "Publishing {} result for {}/{}",
                    "task".yellow(),
                    task_metadata.file_id,
                    task_metadata.task_id
                );

                // TODO: will get better token count from `TaskWorkerOutput`
                let token_count = result.len();
                let payload = TaskResponsePayload {
                    result: Some(result),
                    error: None,
                    file_id: task_metadata.file_id,
                    task_id: task_metadata.task_id,
                    row_id: task_output.row_id,
                    model: task_metadata.model_name,
                    stats: task_output
                        .stats
                        .record_published_at()
                        .record_token_count(token_count),
                };
                let payload_str =
                    serde_json::to_string(&payload).wrap_err("could not serialize payload")?;

                node.new_message(payload_str, TASK_RESULT_TOPIC)
            }
            Err(err) => {
                // use pretty display string for error logging with causes
                let err_string = format!("{:#}", err);
                log::error!(
                    "Task {}/{} failed: {}",
                    task_metadata.file_id,
                    task_metadata.task_id,
                    err_string
                );

                // prepare error payload
                let error_payload = TaskResponsePayload {
                    result: None,
                    error: Some(err_string),
                    row_id: task_output.row_id,
                    file_id: task_metadata.file_id,
                    task_id: task_metadata.task_id,
                    model: task_metadata.model_name,
                    stats: task_output
                        .stats
                        .record_published_at()
                        .record_token_count(0),
                };
                let error_payload_str = serde_json::to_string(&error_payload)
                    .wrap_err("could not serialize payload")?;

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
