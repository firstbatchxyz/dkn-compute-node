use eyre::{eyre, Context, Result};
use ollama_workflows::Model;
use reqwest::Client;
use serde::Deserialize;
use std::env;

use crate::utils::safe_read_env;

/// [`models.list`](https://ai.google.dev/api/models#method:-models.list) endpoint
const GEMINI_MODELS_API: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const ENV_VAR_NAME: &str = "GEMINI_API_KEY";

/// [Model](https://ai.google.dev/api/models#Model) API object, fields omitted.
#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
#[allow(unused)]
struct GeminiModel {
    name: String,
    version: String,
    // other fields are ignored from API response
}

#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
#[allow(unused)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

/// OpenAI-specific configurations.
#[derive(Debug, Clone, Default)]
pub struct GeminiConfig {
    /// API key, if available.
    api_key: Option<String>,
}

impl GeminiConfig {
    /// Looks at the environment variables for Gemini API key.
    pub fn new() -> Self {
        Self {
            api_key: safe_read_env(env::var(ENV_VAR_NAME)),
        }
    }

    /// Sets the API key for Gemini.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Check if requested models exist & are available in the OpenAI account.
    pub async fn check(&self, models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking Gemini requirements");

        // check API key
        let Some(api_key) = &self.api_key else {
            return Err(eyre!("Gemini API key not found"));
        };

        // fetch models
        let client = Client::new();
        let request = client
            .get(GEMINI_MODELS_API)
            .query(&[("key", api_key)])
            .build()
            .wrap_err("failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("failed to send request")?;

        // parse response
        if response.status().is_client_error() {
            return Err(eyre!(
                "Failed to fetch Gemini models:\n{}",
                response.text().await.unwrap_or_default()
            ));
        }
        let gemini_models = response.json::<GeminiModelsResponse>().await?;

        // check if models exist and select those that are available
        let mut available_models = Vec::new();
        for requested_model in models {
            if !gemini_models.models.iter().any(|gemini_model| {
                // a gemini model name in API response is given as `models/{baseModelId}-{version}`
                // the model name in Workflows can include the version as well, so best bet is to check prefix
                // ignoring the `models/` part
                gemini_model
                    .name
                    .trim_start_matches("models/")
                    .starts_with(&requested_model.to_string())
            }) {
                log::warn!(
                    "Model {} not found in your Gemini account, ignoring it.",
                    requested_model
                );
            } else {
                available_models.push(requested_model);
            }
        }

        log::info!(
            "Gemini checks are finished, using models: {:#?}",
            available_models
        );

        Ok(available_models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Gemini API key"]
    async fn test_gemini_check() {
        let _ = dotenvy::dotenv(); // read api key
        assert!(env::var(ENV_VAR_NAME).is_ok(), "should have api key");

        let models = vec![
            Model::Gemini10Pro,
            Model::Gemini15ProExp0827,
            Model::Gemini15Flash,
            Model::Gemini15Pro,
        ];
        let res = GeminiConfig::new().check(models.clone()).await;
        assert_eq!(res.unwrap(), models);

        env::set_var(ENV_VAR_NAME, "i-dont-work");
        let res = GeminiConfig::new().check(vec![]).await;
        assert!(res.is_err());

        env::remove_var(ENV_VAR_NAME);
        let res = GeminiConfig::new().check(vec![]).await;
        assert!(res.is_err());
    }
}
