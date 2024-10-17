use eyre::{eyre, Context, Result};
use ollama_workflows::Model;
use reqwest::Client;
use serde::Deserialize;

use crate::utils::safe_read_env;

// curl https://generativelanguage.googleapis.com/v1beta/models?key=$GOOGLE_API_KEY
const GEMINI_MODELS_API: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const ENV_VAR_NAME: &str = "GEMINI_API_KEY";

/// [Model](https://ai.google.dev/api/models#Model) API object.
#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
#[allow(unused)]
struct GeminiModel {
    name: String,
    baseModelId: String,
    version: String,
    displayName: String,
    description: String,
    inputTokenLimit: u64,
    outputTokenLimit: u64,
    supportedGenerationMethods: Vec<String>,
    temperature: f64,
    maxTemperature: f64,
    topP: f64,
    topK: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
#[allow(unused)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
    #[allow(unused)]
    nextPageToken: String,
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
            api_key: safe_read_env(std::env::var(ENV_VAR_NAME)),
        }
    }

    /// Sets the API key for Gemini.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Check if requested models exist & are available in the OpenAI account.
    pub async fn check(&self, models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking OpenAI requirements");

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
            if !gemini_models
                .models
                .iter()
                .any(|m| m.baseModelId == requested_model.to_string())
            {
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
        let config = GeminiConfig::new();
        let res = config.check(vec![]).await;
        println!("Result: {}", res.unwrap_err());
    }
}
