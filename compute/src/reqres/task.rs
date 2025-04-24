use colored::Colorize;
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::payloads::{TaskRequestPayload, TaskResponsePayload, TaskStats, TASK_RESULT_TOPIC};
use dkn_utils::DriaMessage;
use dkn_workflows::{Executor, ModelProvider, TaskWorkflow};
use eyre::{eyre, Context, Result};

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
        node: &mut DriaComputeNode,
        compute_message: &DriaMessage,
        channel: ResponseChannel<Vec<u8>>,
    ) -> Result<(TaskWorkerInput, TaskWorkerMetadata)> {
        // parse payload
        let task = compute_message
            .parse_payload::<TaskRequestPayload<TaskWorkflow>>()
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

        // read model / provider from the task
        let model = node
            .config
            .workflows
            .get_any_matching_model(vec![task.input.model])?; // FIXME: dont use vector here
        let model_name = model.to_string(); // get model name, we will pass it in payload
        log::info!("Using model {} for task {}", model_name, task.task_id);

        // prepare workflow executor
        let (executor, batchable) = if model.provider() == ModelProvider::Ollama {
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

        // get workflow as well
        let workflow = task.input.workflow;

        let task_input = TaskWorkerInput {
            executor,
            workflow,
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

#[cfg(test)]
mod tests {
    use dkn_utils::payloads::TaskRequestPayload;
    use dkn_workflows::TaskWorkflow;

    #[test]
    fn test_serialize() {
        // FIXME: remove this
        let buf = "{\"taskId\":\"123456789--abcdef\",\"deadline\":\"2026-04-24 13:04:13.317444 UTC\",\"publicKey\":\"02e881b263932ad70f6082be1169894925d867b75af6a336daa3ed106f3d53621b\",\"input\":{\"model\":[\"gemini-1.5-flash\"],\"workflow\":{\"config\":{\"max_steps\":10,\"max_time\":250,\"tools\":[\"\"]},\"tasks\":[{\"id\":\"A\",\"name\":\"\",\"description\":\"\",\"operator\":\"generation\",\"messages\":[{\"role\":\"user\",\"content\":\"Write a 4 paragraph poem about Julius Caesar.\"}],\"outputs\":[{\"type\":\"write\",\"key\":\"result\",\"value\":\"__result\"}]},{\"id\":\"__end\",\"name\":\"end\",\"description\":\"End of the task\",\"operator\":\"end\",\"messages\":[{\"role\":\"user\",\"content\":\"End of the task\"}]}],\"steps\":[{\"source\":\"A\",\"target\":\"__end\"}],\"return_value\":{\"input\":{\"type\":\"read\",\"key\":\"result\"}}}}}";
        println!("buf: {}", chrono::Utc::now());
        let _payload: TaskRequestPayload<TaskWorkflow> =
            serde_json::from_str(buf).expect("should be deserializable");
    }
}
