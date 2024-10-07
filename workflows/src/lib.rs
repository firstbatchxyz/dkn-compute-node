use async_trait::async_trait;
use eyre::Result;

mod models;
pub use models::ModelConfig;

/// Ollama configurations & service checks
mod ollama;
pub(crate) use ollama::OllamaConfig;

/// OpenAI configurations & service checks
mod openai;
pub(crate) use openai::OpenAIConfig;

/// Extension trait for model providers to check if they are ready, and describe themselves.
#[async_trait]
pub trait ProvidersExt {
    const PROVIDER_NAME: &str;

    /// Ensures that the required provider is online & ready.
    async fn check_service(&self) -> Result<()>;
}
