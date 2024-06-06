use langchain_rust::language_models::llm::LLM;
use tokio_util::sync::CancellationToken;

use super::ollama::create_ollama;
use super::openai::create_openai;

#[derive(Debug, Default)]
pub enum ModelProvider {
    #[default]
    Ollama,
    OpenAI,
}

impl From<String> for ModelProvider {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str().trim() {
            "ollama" => Self::Ollama,
            "openai" => Self::OpenAI,
            _ => {
                log::warn!("Unknown LLM type: {}, defaulting.", value);
                Self::default()
            }
        }
    }
}

impl From<&ModelProvider> for String {
    fn from(value: &ModelProvider) -> Self {
        match value {
            ModelProvider::Ollama => "Ollama".to_string(),
            ModelProvider::OpenAI => "OpenAI".to_string(),
        }
    }
}

impl std::fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

/// Creates an LLM of the given type, which is a LangChain object.
///
/// The respective setups of the LLMs are done within this function,
/// e.g. Ollama will pull the model if it does not exist locally.
pub async fn create_llm(
    llm: ModelProvider,
    model: String,
    cancellation: CancellationToken,
) -> Result<Box<dyn LLM>, String> {
    match llm {
        ModelProvider::Ollama => {
            let client = create_ollama(cancellation, model).await?;
            Ok(Box::new(client))
        }
        ModelProvider::OpenAI => {
            let client = create_openai(model);
            Ok(Box::new(client))
        }
    }
}
