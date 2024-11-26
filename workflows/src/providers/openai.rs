use eyre::{eyre, Context, Result};
use ollama_workflows::Model;
use reqwest::Client;
use serde::Deserialize;
use std::env;

use crate::utils::safe_read_env;

const ENV_VAR_NAME: &str = "OPENAI_API_KEY";

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

    /// Returns the list of model names available to this account.
    pub async fn check(&self, models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking OpenAI requirements");

        // check API key
        let Some(api_key) = &self.api_key else {
            return Err(eyre!("OpenAI API key not found"));
        };

        // check if models exist within the account and select those that are available
        let openai_model_names = self.fetch_models(api_key).await?;
        let mut available_models = Vec::new();
        for requested_model in models {
            // check if model exists
            if !openai_model_names.contains(&requested_model.to_string()) {
                log::warn!(
                    "Model {} not found in your OpenAI account, ignoring it.",
                    requested_model
                );
                continue;
            }

            // make a dummy request
            if let Err(err) = self.dummy_request(api_key, &requested_model).await {
                log::warn!(
                    "Model {} failed dummy request, ignoring it: {}",
                    requested_model,
                    err
                );
                continue;
            }

            available_models.push(requested_model)
        }

        // log results
        if available_models.is_empty() {
            log::warn!("OpenAI checks are finished, no available models found.",);
        } else {
            log::info!(
                "OpenAI checks are finished, using models: {:#?}",
                available_models
            );
        }

        Ok(available_models)
    }

    /// Fetches the list of models available in the OpenAI account.
    async fn fetch_models(&self, api_key: &str) -> Result<Vec<String>> {
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

        let client = Client::new();
        let request = client
            .get("https://api.openai.com/v1/models")
            .header("Authorization", format!("Bearer {}", api_key))
            .build()
            .wrap_err("failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("failed to send request")?;

        // parse response
        if !response.status().is_success() {
            Err(eyre!(
                "Failed to fetch OpenAI models:\n{}",
                response
                    .text()
                    .await
                    .unwrap_or("Could not get error text as well".to_string())
            ))
        } else {
            let openai_models = response.json::<OpenAIModelsResponse>().await?;
            Ok(openai_models.data.into_iter().map(|m| m.id).collect())
        }
    }

    /// Makes a dummy request to the OpenAI API to check if the model is available & has credits.
    async fn dummy_request(&self, api_key: &str, model: &Model) -> Result<()> {
        log::debug!("Making a dummy request with: {}", model);
        let client = Client::new();
        let request = client
            .post("https://api.openai.com/v1/chat/completions")
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
                "Failed to make OpenAI chat request:\n{}",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires OpenAI API key"]
    async fn test_openai_check() {
        let _ = dotenvy::dotenv(); // read api key
        assert!(env::var(ENV_VAR_NAME).is_ok(), "should have api key");
        env::set_var("RUST_LOG", "none,dkn_workflows=debug");
        let _ = env_logger::builder().is_test(true).try_init();

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
