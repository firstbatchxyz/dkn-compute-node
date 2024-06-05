use langchain_rust::language_models::llm::LLM;
use tokio_util::sync::CancellationToken;

use super::ollama::create_ollama;
use super::openai::create_openai;

#[derive(Debug, Default)]
pub enum LLMType {
    #[default]
    Ollama,
    OpenAI,
}

impl From<String> for LLMType {
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

impl From<&LLMType> for String {
    fn from(value: &LLMType) -> Self {
        match value {
            LLMType::Ollama => "Ollama".to_string(),
            LLMType::OpenAI => "OpenAI".to_string(),
        }
    }
}

impl std::fmt::Display for LLMType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

/// Creates an LLM of the given type, which is a LangChain object.
///
/// The respective setups of the LLMs are done within this function,
/// e.g. Ollama will pull the model if it does not exist locally.
pub async fn create_llm(
    llm: LLMType,
    cancellation: CancellationToken,
) -> Result<Box<dyn LLM>, String> {
    match llm {
        LLMType::Ollama => {
            let client = create_ollama(cancellation.clone()).await?;
            Ok(Box::new(client))
        }
        LLMType::OpenAI => {
            let client = create_openai();
            Ok(Box::new(client))
        }
    }
}
