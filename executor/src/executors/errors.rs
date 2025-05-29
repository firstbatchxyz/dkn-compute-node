#![allow(unused)]

use crate::{Model, ModelProvider};
use rig::completion::{CompletionError, PromptError};

#[derive(Debug, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum DriaExecutorError {
    #[error("Model {0} is not a valid model.")]
    InvalidModel(String),
    #[error("Model {0} not found in your configuration.")]
    ModelNotSupported(Model),
    #[error("Provider {0} not found in your configuration")]
    ProviderNotSupported(ModelProvider),

    /// A generic error that wraps a [`rig::completion::PromptError`] in string form.
    #[error("Rig error: {0}")]
    RigError(String),
    /// A sub-type of `PromptError` that succesfully parses the error from the provider.
    ///
    /// It is parsed from `PrompError(ProviderError(String))`.
    #[error("{provider} error ({code}): {message}")]
    ProviderError {
        /// Not necessarily an HTTP status code, but a code that the provider uses to identify the error.
        ///
        /// For example, OpenAI uses a string code like "invalid_request_error".
        code: String,
        /// The error message returned by the provider.
        ///
        /// May contain additional information about the error.
        message: String,
        /// The provider that returned the error.
        ///
        /// Do we need it?
        provider: ModelProvider,
    },
}

/// Maps a [`PromptError`] to a [`DriaExecutorError`] with respect to the given provider.
pub fn map_prompt_error(provider: &ModelProvider, err: PromptError) -> DriaExecutorError {
    if let PromptError::CompletionError(CompletionError::ProviderError(err_inner)) = &err {
        // all the body's below have an `error` field
        #[derive(Clone, serde::Deserialize)]
        struct ErrorObject<T> {
            error: T,
        }

        match provider {
            ModelProvider::Gemini => {
                /// A Gemini API error object.
                ///
                /// See their Go [client for reference](https://github.com/googleapis/go-genai/blob/main/api_client.go#L273).
                #[derive(Clone, serde::Deserialize)]
                pub struct GeminiError {
                    code: u32,
                    message: String,
                    status: String,
                }

                serde_json::from_str::<ErrorObject<GeminiError>>(err_inner).map(
                    |ErrorObject {
                         error: gemini_error,
                     }| DriaExecutorError::ProviderError {
                        code: format!("{} ({})", gemini_error.code, gemini_error.status),
                        message: gemini_error.message,
                        provider: ModelProvider::Gemini,
                    },
                )
            }
            ModelProvider::OpenAI => {
                /// An OpenAI error object.
                ///
                /// See their Go [client for reference](https://github.com/openai/openai-go/blob/main/internal/apierror/apierror.go#L17).
                #[derive(Clone, serde::Deserialize)]
                pub struct OpenAIError {
                    code: String,
                    message: String,
                }

                serde_json::from_str::<ErrorObject<OpenAIError>>(err_inner).map(
                    |ErrorObject {
                         error: openai_error,
                     }| DriaExecutorError::ProviderError {
                        code: openai_error.code,
                        message: openai_error.message,
                        provider: ModelProvider::OpenAI,
                    },
                )
            }
            ModelProvider::OpenRouter => {
                /// An OpenRouter error object.
                ///
                /// See [their documentation](https://openrouter.ai/docs/api-reference/errors).
                #[derive(Clone, serde::Deserialize)]
                pub struct OpenRouterError {
                    code: u32,
                    message: String,
                }

                serde_json::from_str::<ErrorObject<OpenRouterError>>(err_inner).map(
                    |ErrorObject {
                         error: openrouter_error,
                     }| {
                        DriaExecutorError::ProviderError {
                            code: openrouter_error.code.to_string(),
                            message: openrouter_error.message,
                            provider: ModelProvider::OpenRouter,
                        }
                    },
                )
            }
            ModelProvider::Ollama => serde_json::from_str::<ErrorObject<String>>(err_inner).map(
                |ErrorObject {
                     error: ollama_error,
                 }| {
                    DriaExecutorError::ProviderError {
                        code: "ollama".to_string(),
                        message: ollama_error,
                        provider: ModelProvider::Ollama,
                    }
                },
            ),
        }
        // if we couldn't parse it, just return a generic prompt error
        .unwrap_or(DriaExecutorError::RigError(err.to_string()))
    } else {
        // not a provider error, fallback to generic prompt error
        DriaExecutorError::RigError(err.to_string())
    }
}
