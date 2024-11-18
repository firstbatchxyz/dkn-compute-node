use eyre::{eyre, Context, Result};
use ollama_workflows::Model;
use reqwest::Client;
use std::env;

use crate::utils::safe_read_env;

const ENV_VAR_NAME: &str = "OPENROUTER_API_KEY";

/// OpenRouter-specific configurations.
#[derive(Debug, Clone, Default)]
pub struct OpenRouterConfig {
    /// API key, if available.
    api_key: Option<String>,
}

impl OpenRouterConfig {
    /// Looks at the environment variables for OpenRouter API key.
    pub fn new() -> Self {
        Self {
            api_key: safe_read_env(env::var(ENV_VAR_NAME)),
        }
    }

    /// Sets the API key for OpenRouter.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Checks if the API key exists.
    pub async fn check(&self, external_models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking OpenRouter API key");

        // check API key
        let Some(api_key) = &self.api_key else {
            return Err(eyre!("OpenRouter API key not found"));
        };

        // make a dummy request with existing models
        let mut available_models = Vec::new();
        for requested_model in external_models {
            // make a dummy request
            if let Err(err) = self.dummy_request(api_key.as_str(), &requested_model).await {
                log::warn!(
                    "Model {} failed dummy request, ignoring it: {}",
                    requested_model,
                    err
                );
                continue;
            }

            available_models.push(requested_model)
        }

        Ok(available_models)
    }

    /// Makes a dummy request to the OpenAI API to check if the model is available & has credits.
    async fn dummy_request(&self, api_key: &str, model: &Model) -> Result<()> {
        log::debug!("Making a dummy request with: {}", model);
        let client = Client::new();
        let request = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!({
                  "model": model.to_string(),
                  "messages": [
                    {
                      "role": "user",
                      "content": "What is 2+2?"
                    }
                  ]
                })
                .to_string(),
            )
            .build()
            .wrap_err("failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("failed to send request")?;

        // ensure response is ok
        if !response.status().is_success() {
            return Err(eyre!(
                "Failed to make OpenRouter chat request:\n{}",
                response
                    .text()
                    .await
                    .unwrap_or("Could not get error text as well".to_string())
            ));
        }
        log::debug!("Dummy request successful for model {}", model);

        Ok(())
    }
}

// FIXME: add tests
