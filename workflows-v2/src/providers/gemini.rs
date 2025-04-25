use eyre::{eyre, Context, Result};
use reqwest::Client;
use rig::{
    completion::{Chat, PromptError},
    providers::gemini,
};
use serde::Deserialize;

use crate::{Model, TaskBody};

/// OpenAI-specific configurations.
#[derive(Clone)]
pub struct GeminiProvider {
    api_key: String,
    client: gemini::Client,
}

impl GeminiProvider {
    /// Looks at the environment variables for Gemini API key.
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: gemini::Client::new(api_key),
        }
    }

    /// Creates a new client using the API key in `GEMINI_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let api_key = std::env::var("GEMINI_API_KEY")?;
        Ok(Self::new(&api_key))
    }

    pub async fn execute(&self, task: TaskBody) -> Result<String, PromptError> {
        let mut model = self.client.agent(&task.model.to_string());
        if let Some(preamble) = task.preamble {
            model = model.preamble(&preamble);
        }

        let agent = model.build();

        agent.chat(task.prompt, task.chat_history).await
    }

    /// Check if requested models exist & are available in the OpenAI account.
    pub async fn check(&self, models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking Gemini requirements");

        // check if models exist and select those that are available
        let gemini_models_names = self.fetch_models().await?;
        let mut available_models = Vec::new();
        for requested_model in models {
            // check if model exists
            if !gemini_models_names
                .iter()
                .any(|model| model.starts_with(&requested_model.to_string()))
            {
                log::warn!(
                    "Model {} not found in your Gemini account, ignoring it.",
                    requested_model
                );
                continue;
            }

            // make a dummy request
            if let Err(err) = self
                .execute(TaskBody::new_prompt("What is 2 + 2?", requested_model))
                .await
            {
                log::warn!(
                    "Model {} failed dummy request, ignoring it: {}",
                    requested_model,
                    err
                );
                continue;
            }

            available_models.push(requested_model);
        }

        // log results
        if available_models.is_empty() {
            log::warn!("Gemini checks are finished, no available models found.",);
        } else {
            log::info!(
                "Gemini checks are finished, using models: {:#?}",
                available_models
            );
        }

        Ok(available_models)
    }

    /// Returns the list of models available to this account.
    ///
    /// A gemini model name in API response is given as `models/{baseModelId}-{version}`
    /// the model name in Workflows can include the version as well, so best bet is to check prefix
    /// ignoring the `models/` part.
    async fn fetch_models(&self) -> Result<Vec<String>> {
        /// [Model](https://ai.google.dev/api/models#Model) API object, fields omitted.
        #[derive(Debug, Clone, Deserialize)]
        struct GeminiModel {
            name: String,
            // other fields are ignored from API response
        }

        #[derive(Debug, Clone, Deserialize)]
        struct GeminiModelsResponse {
            models: Vec<GeminiModel>,
        }

        // fetch models
        let client = Client::new();
        let request = client
            // [`models.list`](https://ai.google.dev/api/models#method:-models.list) endpoint
            .get("https://generativelanguage.googleapis.com/v1beta/models")
            .query(&[("key", &self.api_key)])
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

        Ok(gemini_models
            .models
            .into_iter()
            .map(|model| model.name.trim_start_matches("models/").to_string())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Gemini API key"]
    async fn test_gemini_check() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_workflows", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        let _ = dotenvy::dotenv(); // read api key

        let models = vec![Model::Gemini15Flash, Model::Gemini15Pro];
        let res = GeminiProvider::from_env()
            .unwrap()
            .check(models.clone())
            .await;
        assert_eq!(res.unwrap(), models);

        let res = GeminiProvider::new("i-dont-work").check(vec![]).await;
        assert!(res.is_err());
    }
}
