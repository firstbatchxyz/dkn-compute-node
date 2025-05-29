use crate::ModelProvider;
use rig::completion::PromptError;
use std::collections::HashSet;

mod errors;
pub use errors::DriaExecutorError;

mod ollama;
use ollama::OllamaClient;

mod openai;
use openai::OpenAIClient;

mod gemini;
use gemini::GeminiClient;

mod openrouter;
use openrouter::OpenRouterClient;

/// A wrapper enum for all model providers.
#[derive(Clone)]
pub enum DriaExecutor {
    Ollama(OllamaClient),
    OpenAI(OpenAIClient),
    Gemini(GeminiClient),
    OpenRouter(OpenRouterClient),
}

impl DriaExecutor {
    /// Creates a new executor for the given provider using the API key in the environment variables.
    pub fn new_from_env(provider: ModelProvider) -> Result<Self, std::env::VarError> {
        match provider {
            ModelProvider::Ollama => OllamaClient::from_env().map(DriaExecutor::Ollama),
            ModelProvider::OpenAI => OpenAIClient::from_env().map(DriaExecutor::OpenAI),
            ModelProvider::Gemini => GeminiClient::from_env().map(DriaExecutor::Gemini),
            ModelProvider::OpenRouter => OpenRouterClient::from_env().map(DriaExecutor::OpenRouter),
        }
    }

    /// Executes the given task using the appropriate provider.
    pub async fn execute(&self, task: crate::TaskBody) -> Result<String, PromptError> {
        match self {
            DriaExecutor::Ollama(provider) => provider.execute(task).await,
            // .map_err(|e| map_prompt_error(&ModelProvider::Ollama, e)),
            DriaExecutor::OpenAI(provider) => provider.execute(task).await,
            // .map_err(|e| map_prompt_error(&ModelProvider::OpenAI, e)),
            DriaExecutor::Gemini(provider) => provider.execute(task).await,
            // .map_err(|e| map_prompt_error(&ModelProvider::Gemini, e)),
            DriaExecutor::OpenRouter(provider) => provider.execute(task).await,
            // .map_err(|e| map_prompt_error(&ModelProvider::OpenRouter, e)),
        }
    }

    /// Checks if the requested models exist and are available in the provider's account.
    ///
    /// For Ollama in particular, it also checks if the models are performant enough.
    pub async fn check(&self, models: &mut HashSet<crate::Model>) -> eyre::Result<()> {
        match self {
            DriaExecutor::Ollama(provider) => provider.check(models).await,
            DriaExecutor::OpenAI(provider) => provider.check(models).await,
            DriaExecutor::Gemini(provider) => provider.check(models).await,
            DriaExecutor::OpenRouter(provider) => provider.check(models).await,
        }
    }
}
