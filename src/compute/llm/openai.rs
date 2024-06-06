use std::env;

use langchain_rust::llm::openai::OpenAI;
use langchain_rust::llm::OpenAIConfig;

use crate::config::constants::*;

/// Creates an OpenAI langchain client.
///
/// Will check for the following environment variables:
///
/// - `OPENAI_API_BASE`
/// - `OPENAI_API_KEY`
/// - `OPENAI_ORG_ID`
/// - `OPENAI_PROJECT_ID`
pub fn create_openai(model: String) -> OpenAI<OpenAIConfig> {
    let mut config = OpenAIConfig::default();

    if let Ok(api_base) = env::var(OPENAI_API_BASE_URL) {
        config = config.with_api_base(api_base);
    }
    if let Ok(api_key) = env::var(OPENAI_API_KEY) {
        config = config.with_api_key(api_key);
    }
    if let Ok(org_id) = env::var(OPENAI_ORG_ID) {
        config = config.with_org_id(org_id);
    }
    if let Ok(project_id) = env::var(OPENAI_PROJECT_ID) {
        config = config.with_project_id(project_id);
    }

    OpenAI::new(config).with_model(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use langchain_rust::language_models::llm::LLM;

    #[tokio::test]
    #[ignore] // cargo test --package dkn-compute --lib --all-features -- compute::openai::tests::test_openai --exact --show-output --ignored
    async fn test_openai() {
        let value = "FOOBARFOOBAR"; // use with your own key, with caution
        env::set_var(OPENAI_API_KEY, value);

        let openai = create_openai("gpt-3.5-turbo".to_string());

        let prompt = "Once upon a time, in a land far away, there was a dragon.";
        let response = openai
            .invoke(prompt)
            .await
            .expect("Should generate response");
        println!("{}", response);
    }
}
