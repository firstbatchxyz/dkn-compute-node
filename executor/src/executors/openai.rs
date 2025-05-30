use std::collections::{HashMap, HashSet};

use dkn_utils::payloads::SpecModelPerformance;
use eyre::{eyre, Context, Result};
use reqwest::Client;
use rig::{
    completion::{Chat, PromptError},
    providers::openai,
};
use serde::Deserialize;

use crate::{Model, TaskBody};

/// OpenAI-specific configurations.
#[derive(Clone)]
pub struct OpenAIClient {
    /// API key, if available.
    api_key: String,
    /// Underlying OpenAI client from [`rig`].
    client: openai::Client,
}

impl OpenAIClient {
    /// Looks at the environment variables for OpenAI API key.
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: openai::Client::new(api_key),
        }
    }

    /// Creates a new OpenAI client using the API key in `OPENAI_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let api_key = std::env::var("OPENAI_API_KEY")?;
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

    /// Returns the list of model names available to this account.
    pub async fn check(
        &self,
        models: &mut HashSet<Model>,
    ) -> Result<HashMap<Model, SpecModelPerformance>> {
        let mut models_to_remove = Vec::new();
        let mut model_performances = HashMap::new();
        log::info!("Checking OpenAI requirements");

        // check if models exist within the account and select those that are available
        let openai_model_names = self.fetch_models().await?;
        for model in models.iter().cloned() {
            // check if model exists
            if !openai_model_names.contains(&model.to_string()) {
                log::warn!(
                    "Model {} not found in your OpenAI account, ignoring it.",
                    model
                );
                models_to_remove.push(model);
                model_performances.insert(model, SpecModelPerformance::NotFound);
                continue;
            }

            // if it exists, make a dummy request
            if let Err(err) = self
                .execute(TaskBody::new_prompt("What is 2 + 2?", model))
                .await
            {
                log::warn!("Model {} failed dummy request, ignoring it: {}", model, err);
                models_to_remove.push(model);
                model_performances.insert(model, SpecModelPerformance::ExecutionFailed);
                continue;
            }

            // record the performance of the model
            model_performances.insert(model, SpecModelPerformance::Passed);
        }

        // remove models that are not available
        for model in models_to_remove.iter() {
            models.remove(model);
        }

        // log results
        if models.is_empty() {
            log::warn!("OpenAI checks are finished, no available models found.",);
        } else {
            log::info!("OpenAI checks are finished, using models: {:#?}", models);
        }

        Ok(model_performances)
    }

    /// Fetches the list of models available in the OpenAI account.
    async fn fetch_models(&self) -> Result<Vec<String>> {
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
            .header("Authorization", format!("Bearer {}", self.api_key))
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
                    .unwrap_or("could not get error text as well".to_string())
            ))
        } else {
            let openai_models = response.json::<OpenAIModelsResponse>().await?;
            Ok(openai_models.data.into_iter().map(|m| m.id).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires OpenAI API key"]
    async fn test_openai_check() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_executor", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        let _ = dotenvy::dotenv(); // read api key

        let initial_models = [Model::GPT4o, Model::GPT4oMini];
        let mut models = HashSet::from_iter(initial_models);
        OpenAIClient::from_env()
            .unwrap()
            .check(&mut models)
            .await
            .unwrap();
        assert_eq!(models.len(), initial_models.len());

        let res = OpenAIClient::new("i-dont-work")
            .check(&mut Default::default())
            .await;
        assert!(res.is_err());
    }
}
