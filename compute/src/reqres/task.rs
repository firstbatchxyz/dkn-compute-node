use colored::Colorize;
use dkn_executor::{CompletionError, ModelProvider, PromptError, TaskBody};
use dkn_p2p::libp2p::request_response::ResponseChannel;
use dkn_utils::payloads::{
    TaskError, TaskRequestPayload, TaskResponsePayload, TaskStats, TASK_RESULT_TOPIC,
};
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
        // parse this in two-steps so that if something goes wrong we know the task id
        let task = compute_message
            .parse_payload::<TaskRequestPayload<serde_json::Value>>()
            .wrap_err("could not parse task request payload")?;
        let task_body = match serde_json::from_value::<TaskBody>(task.input) {
            Ok(task_body) => task_body,
            Err(err) => {
                log::error!(
                    "Task {}/{} failed due to parsing error: {err}",
                    task.file_id,
                    task.row_id,
                );

                // prepare error payload
                let error_payload = TaskResponsePayload {
                    result: None,
                    error: Some(TaskError::ParseError(err.to_string())),
                    row_id: task.row_id,
                    file_id: task.file_id,
                    task_id: task.task_id,
                    model: "<n/a>".to_string(), // no model available due to parsing error
                    stats: TaskStats::new(),
                };

                let error_payload_str = serde_json::to_string(&error_payload)
                    .wrap_err("could not serialize payload")?;

                // respond through the channel to notify about the parsing error
                let response = node.new_message(error_payload_str, TASK_RESULT_TOPIC);
                node.p2p.respond(response.into(), channel).await?;

                // return with error
                eyre::bail!("could not parse task body: {err}")
            }
        };

        let stats = TaskStats::new().record_received_at();
        log::info!(
            "Handling {} {} with model {}",
            "task".yellow(),
            task.row_id,
            task_body.model.to_string().yellow()
        );

        // check if the model is available in this node, if so
        // it will return an executor that can run this model
        let executor = node.config.executors.get_executor(&task_body.model).await?;

        let task_metadata = TaskWorkerMetadata {
            task_id: task.task_id,
            file_id: task.file_id,
            model: task_body.model,
            channel,
        };
        let task_input = TaskWorkerInput {
            executor,
            task: task_body,
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
                    task_output.row_id
                );

                // TODO: will get better token count from `TaskWorkerOutput`
                let token_count = result.len();
                let payload = TaskResponsePayload {
                    result: Some(result),
                    error: None,
                    file_id: task_metadata.file_id,
                    task_id: task_metadata.task_id,
                    row_id: task_output.row_id,
                    model: task_metadata.model.to_string(),
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
                log::error!(
                    "Task {}/{} failed: {:#}",
                    task_metadata.file_id,
                    task_output.row_id,
                    err
                );

                // prepare error payload
                let error_payload = TaskResponsePayload {
                    result: None,
                    error: Some(map_prompt_error_to_task_error(
                        task_metadata.model.provider(),
                        err,
                    )),
                    row_id: task_output.row_id,
                    file_id: task_metadata.file_id,
                    task_id: task_metadata.task_id,
                    model: task_metadata.model.to_string(),
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

/// Maps a [`PromptError`] to a [`TaskError`] with respect to the given provider.
fn map_prompt_error_to_task_error(provider: ModelProvider, err: PromptError) -> TaskError {
    match &err {
        // if the error is a provider error, we can try to parse it
        PromptError::CompletionError(CompletionError::ProviderError(err_inner)) => {
            /// A wrapper for `{ error: T }` to match the provider error format.
            #[derive(Clone, serde::Deserialize)]
            struct ErrorObject<T> {
                error: T,
            }

            match provider {
                ModelProvider::Gemini => {
                    /// Gemini API [error object](https://github.com/googleapis/go-genai/blob/main/api_client.go#L273).
                    #[derive(Clone, serde::Deserialize)]
                    pub struct GeminiError {
                        code: u32,
                        message: String,
                        status: String,
                    }

                    serde_json::from_str::<ErrorObject<GeminiError>>(err_inner).map(
                        |ErrorObject {
                             error: gemini_error,
                         }| TaskError::ProviderError {
                            code: format!("{} ({})", gemini_error.code, gemini_error.status),
                            message: gemini_error.message,
                            provider: provider.to_string(),
                        },
                    )
                }
                ModelProvider::OpenAI => {
                    /// OpenAI API [error object](https://github.com/openai/openai-go/blob/main/internal/apierror/apierror.go#L17).
                    #[derive(Clone, serde::Deserialize)]
                    pub struct OpenAIError {
                        code: String,
                        message: String,
                    }

                    serde_json::from_str::<ErrorObject<OpenAIError>>(err_inner).map(
                        |ErrorObject {
                             error: openai_error,
                         }| TaskError::ProviderError {
                            code: openai_error.code,
                            message: openai_error.message,
                            provider: provider.to_string(),
                        },
                    )
                }
                ModelProvider::OpenRouter => {
                    /// OpenRouter API [error object](https://openrouter.ai/docs/api-reference/errors).
                    #[derive(Clone, serde::Deserialize)]
                    pub struct OpenRouterError {
                        code: u32,
                        message: String,
                    }

                    serde_json::from_str::<ErrorObject<OpenRouterError>>(err_inner).map(
                        |ErrorObject {
                             error: openrouter_error,
                         }| {
                            TaskError::ProviderError {
                                code: openrouter_error.code.to_string(),
                                message: openrouter_error.message,
                                provider: provider.to_string(),
                            }
                        },
                    )
                }
                ModelProvider::Ollama => serde_json::from_str::<ErrorObject<String>>(err_inner)
                    .map(
                        // Ollama just returns a string error message
                        |ErrorObject {
                             error: ollama_error,
                         }| {
                            // based on the error message, we can come up with out own "dummy" codes
                            let code = if ollama_error.contains("server busy, please try again.") {
                                "server_busy"
                            } else if ollama_error.contains("model requires more system memory") {
                                "model_requires_more_memory"
                            } else if ollama_error.contains("cudaMalloc failed: out of memory") {
                                "cuda_malloc_failed"
                            } else if ollama_error.contains("CUDA error: out of memory") {
                                "cuda_oom"
                            } else if ollama_error.contains("API Error: Too Many Requests") {
                                "api:too_many_requests"
                            } else if ollama_error.contains("API Error: Bad Request") {
                                "api:bad_request"
                            } else if ollama_error.contains("not found, try pulling it first") {
                                "model_not_pulled"
                            } else if ollama_error.contains("Unexpected end of JSON input") {
                                "unexpected_end_of_json"
                            } else {
                                "unknown"
                            };

                            TaskError::ProviderError {
                                code: code.to_string(),
                                message: ollama_error,
                                provider: provider.to_string(),
                            }
                        },
                    ),
            }
            // if we couldn't parse it, just return a generic prompt error
            .unwrap_or(TaskError::ExecutorError(format!(
                "{provider} executor error: {}",
                err_inner.clone()
            )))
        }
        // if its a http error, we can try to parse it as well
        PromptError::CompletionError(CompletionError::HttpError(err_inner)) => {
            TaskError::HttpError(err_inner.to_string())
        }
        // if it's not a completion error, we just return the error as is
        err => TaskError::Other(err.to_string()),
    }
}
