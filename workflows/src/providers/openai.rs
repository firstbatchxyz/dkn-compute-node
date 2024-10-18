use eyre::{eyre, Context, Result};
use ollama_workflows::Model;
use reqwest::Client;
use serde::Deserialize;
use std::env;

use crate::utils::safe_read_env;

const OPENAI_MODELS_API: &str = "https://api.openai.com/v1/models";
const ENV_VAR_NAME: &str = "OPENAI_API_KEY";

/// [Model](https://platform.openai.com/docs/api-reference/models/object) API object, fields omitted.
#[derive(Debug, Clone, Deserialize)]
struct OpenAIModel {
    /// The model identifier, which can be referenced in the API endpoints.
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

/// OpenAI-specific configurations.
#[derive(Debug, Clone, Default)]
pub struct OpenAIConfig {
    /// API key, if available.
    api_key: Option<String>,
}

impl OpenAIConfig {
    /// Looks at the environment variables for OpenAI API key.
    pub fn new() -> Self {
        Self {
            api_key: safe_read_env(env::var(ENV_VAR_NAME)),
        }
    }

    /// Sets the API key for OpenAI.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Check if requested models exist & are available in the OpenAI account.
    pub async fn check(&self, models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking OpenAI requirements");

        // check API key
        let Some(api_key) = &self.api_key else {
            return Err(eyre!("OpenAI API key not found"));
        };

        // fetch models
        let client = Client::new();
        let request = client
            .get(OPENAI_MODELS_API)
            .header("Authorization", format!("Bearer {}", api_key))
            .build()
            .wrap_err("failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("failed to send request")?;

        // parse response
        if response.status().is_client_error() {
            return Err(eyre!(
                "Failed to fetch OpenAI models:\n{}",
                response.text().await.unwrap_or_default()
            ));
        }
        let openai_models = response.json::<OpenAIModelsResponse>().await?;

        // check if models exist and select those that are available
        let mut available_models = Vec::new();
        for requested_model in models {
            if !openai_models
                .data
                .iter()
                .any(|m| m.id == requested_model.to_string())
            {
                log::warn!(
                    "Model {} not found in your OpenAI account, ignoring it.",
                    requested_model
                );
            } else {
                available_models.push(requested_model);
            }
        }

        log::info!(
            "OpenAI checks are finished, using models: {:#?}",
            available_models
        );
        Ok(available_models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires OpenAI API key"]
    async fn test_openai_check() {
        let _ = dotenvy::dotenv(); // read api key
        assert!(env::var(ENV_VAR_NAME).is_ok(), "should have api key");

        let models = vec![Model::GPT4Turbo, Model::GPT4o, Model::GPT4oMini];
        let config = OpenAIConfig::new();
        let res = config.check(models.clone()).await;
        assert_eq!(res.unwrap(), models);

        env::set_var(ENV_VAR_NAME, "i-dont-work");
        let config = OpenAIConfig::new();
        let res = config.check(vec![]).await;
        assert!(res.is_err());

        env::remove_var(ENV_VAR_NAME);
        let config = OpenAIConfig::new();
        let res = config.check(vec![]).await;
        assert!(res.is_err());
    }
}
