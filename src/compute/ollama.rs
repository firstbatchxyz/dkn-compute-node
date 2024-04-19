use crate::config::defaults::{DEFAULT_DKN_OLLAMA_HOST, DEFAULT_DKN_OLLAMA_PORT};
use ollama_rs::{models::ModelInfo, Ollama};

/// A wrapper for the Ollama API.
pub struct OllamaClient(Ollama);

impl OllamaClient {
    pub fn new() -> Self {
        Self(Ollama::new(
            DEFAULT_DKN_OLLAMA_HOST.to_string(),
            DEFAULT_DKN_OLLAMA_PORT.parse().unwrap(),
        ))
    }

    /// TODO: ask a model for data synthesis
    pub fn ask_synthesis_prompt(prompt: String) -> String {
        unimplemented!();
    }

    /// TODO: ask a model for result validations
    pub fn validate_results(prompt: String) -> String {
        unimplemented!();
    }
}
