use dkn_utils::payloads::SpecModelPerformance;
use eyre::{eyre, Context, Result};
use reqwest::Client;
use rig::{
    completion::{Chat, PromptError},
    providers::gemini,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::{Model, TaskBody};

/// OpenAI-specific configurations.
#[derive(Clone)]
pub struct GeminiClient {
    api_key: String,
    client: gemini::Client,
}

impl GeminiClient {
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
    pub async fn check(
        &self,
        models: &mut HashSet<Model>,
    ) -> Result<HashMap<Model, SpecModelPerformance>> {
        let mut models_to_remove = Vec::new();
        let mut model_performances = HashMap::new();
        log::info!("Checking Gemini requirements");

        // check if models exist and select those that are available
        let gemini_models_names = self.fetch_models().await?;
        for requested_model in models.iter().cloned() {
            // check if model exists
            if !gemini_models_names
                .iter()
                // due to weird naming of models in Gemini API, we need to check prefix
                .any(|model| model.starts_with(&requested_model.to_string()))
            {
                log::warn!(
                    "Model {} not found in your Gemini account, ignoring it.",
                    requested_model
                );
                models_to_remove.push(requested_model);
                model_performances.insert(requested_model, SpecModelPerformance::NotFound);
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
                models_to_remove.push(requested_model);
                model_performances.insert(requested_model, SpecModelPerformance::ExecutionFailed);
                continue;
            }

            // record the performance of the model
            model_performances.insert(requested_model, SpecModelPerformance::Passed);
        }

        // remove models that are not available
        for model in models_to_remove.iter() {
            models.remove(model);
        }

        Ok(model_performances)
    }

    /// Returns the list of models available to this account.
    ///
    /// A gemini model name in API response is given as `models/{baseModelId}-{version}`
    /// the model name in Dria can include the version as well, so best bet is to check prefix
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
            .filter_module("dkn_executor", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        let _ = dotenvy::dotenv(); // read api key

        let initial_models = [Model::Gemini2_0Flash, Model::Gemini2_5ProExp];
        let mut models = HashSet::from_iter(initial_models);
        GeminiClient::from_env()
            .unwrap()
            .check(&mut models)
            .await
            .unwrap();
        assert_eq!(models.len(), initial_models.len());

        // should give error for bad API key
        let res = GeminiClient::new("i-dont-work")
            .check(&mut HashSet::new())
            .await;
        assert!(res.is_err());
    }
}
