use std::env;

use langchain_rust::language_models::LLMError;
use langchain_rust::llm::OpenAIConfig;
use langchain_rust::{language_models::llm::LLM, llm::openai::OpenAI};

/// A wrapper for the OpenAI API, using LangChain.
///
/// Will check for the following environment variables:
///
/// - `OPENAI_API_BASE`
/// - `OPENAI_API_KEY`
/// - `OPENAI_ORG_ID`
/// - `OPENAI_PROJECT_ID`
#[derive(Clone)]
pub struct OpenAIClient {
    pub(crate) client: OpenAI<OpenAIConfig>,
}

impl Default for OpenAIClient {
    fn default() -> Self {
        Self {
            client: OpenAI::default(),
        }
    }
}

impl OpenAIClient {
    pub fn new() -> Self {
        let mut config = OpenAIConfig::default();

        match env::var("OPENAI_API_BASE") {
            Ok(api_base) => {
                config = config.with_api_base(api_base);
            }
            Err(_) => {}
        }

        match env::var("OPENAI_API_KEY") {
            Ok(api_key) => {
                config = config.with_api_key(api_key);
            }
            Err(_) => {}
        }

        match env::var("OPENAI_ORG_ID") {
            Ok(org_id) => {
                config = config.with_org_id(org_id);
            }
            Err(_) => {}
        }

        match env::var("OPENAI_PROJECT_ID") {
            Ok(project_id) => {
                config = config.with_project_id(project_id);
            }
            Err(_) => {}
        }

        Self {
            client: OpenAI::new(config),
        }
    }

    pub async fn generate(&self, prompt: String) -> Result<String, LLMError> {
        self.client.invoke(prompt.as_str()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // cargo test --package dkn-compute --lib --all-features -- compute::openai::tests::test_openai --exact --show-output --ignored
    async fn test_openai() {
        let value = "FOOBARFOOBAR"; // use with your own key, with caution
        env::set_var("OPENAI_API_KEY", value);

        let openai = OpenAIClient::new();

        let prompt = "Once upon a time, in a land far away, there was a dragon.";
        let response = openai
            .client
            .invoke(prompt)
            .await
            .expect("Should generate response");
        println!("{}", response);
    }
}
