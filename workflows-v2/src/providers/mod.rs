mod ollama;
use std::collections::HashSet;

use ollama::OllamaClient;

mod openai;
use openai::OpenAIClient;

mod gemini;
use gemini::GeminiClient;

mod openrouter;
use openrouter::OpenRouterClient;

use rig::completion::PromptError;

use crate::ModelProvider;

/// A wrapper enum for all workflow providers.
///
/// Exposes the same API for all providers.
#[derive(Clone)]
pub enum DriaWorkflowsProvider {
    Ollama(OllamaClient),
    OpenAI(OpenAIClient),
    Gemini(GeminiClient),
    OpenRouter(OpenRouterClient),
}

impl DriaWorkflowsProvider {
    pub fn new(provider: ModelProvider) -> Self {
        match provider {
            ModelProvider::Ollama => DriaWorkflowsProvider::Ollama(OllamaClient::from_env()),
            ModelProvider::OpenAI => {
                DriaWorkflowsProvider::OpenAI(OpenAIClient::from_env().unwrap())
            }
            ModelProvider::Gemini => {
                DriaWorkflowsProvider::Gemini(GeminiClient::from_env().unwrap())
            }
            ModelProvider::OpenRouter => {
                DriaWorkflowsProvider::OpenRouter(OpenRouterClient::from_env().unwrap())
            }
        }
    }

    pub async fn execute(&self, task: crate::TaskBody) -> Result<String, PromptError> {
        match self {
            DriaWorkflowsProvider::Ollama(provider) => provider.execute(task).await,
            DriaWorkflowsProvider::OpenAI(provider) => provider.execute(task).await,
            DriaWorkflowsProvider::Gemini(provider) => provider.execute(task).await,
            DriaWorkflowsProvider::OpenRouter(provider) => provider.execute(task).await,
        }
    }

    pub async fn check(&self, models: &mut HashSet<crate::Model>) -> eyre::Result<()> {
        match self {
            DriaWorkflowsProvider::Ollama(provider) => provider.check(models).await,
            DriaWorkflowsProvider::OpenAI(provider) => provider.check(models).await,
            DriaWorkflowsProvider::Gemini(provider) => provider.check(models).await,
            DriaWorkflowsProvider::OpenRouter(provider) => Ok(provider.check(models).await),
        }
    }
}
