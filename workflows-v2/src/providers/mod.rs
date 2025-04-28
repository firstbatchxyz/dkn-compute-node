mod ollama;
pub use ollama::OllamaProvider;

mod openai;
pub use openai::OpenAIProvider;

mod gemini;
pub use gemini::GeminiProvider;

mod openrouter;
pub use openrouter::OpenRouterProvider;

use async_trait::async_trait;
use rig::completion::PromptError;

use crate::{Model, TaskBody};

/// Any provider that implements this trait can be used as a workflow provider.
#[async_trait]
pub trait DriaWorkflowProvider: Clone + Send + Sync {
    async fn execute(&self, task: TaskBody) -> Result<String, PromptError>;
    async fn check(&self, models: Vec<Model>) -> eyre::Result<Vec<Model>>;
}
