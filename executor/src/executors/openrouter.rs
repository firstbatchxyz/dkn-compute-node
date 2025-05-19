use std::collections::HashSet;

use eyre::Result;
use rig::completion::{Chat, PromptError};
use rig::providers::openrouter;

use crate::{Model, TaskBody};

/// OpenRouter-specific configurations.
#[derive(Clone)]
pub struct OpenRouterClient {
    client: openrouter::Client,
}

impl OpenRouterClient {
    /// Looks at the environment variables for OpenRouter API key.
    pub fn new(api_key: &str) -> Self {
        Self {
            client: openrouter::Client::new(api_key),
        }
    }

    /// Creates a new client using the API key in `OPENROUTER_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")?;
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

    /// Checks if the API key exists.
    pub async fn check(&self, models: &mut HashSet<Model>) {
        let mut models_to_remove = Vec::new();
        log::info!("Checking OpenRouter API key");

        // make a dummy request with existing models
        for model in models.iter().cloned() {
            if let Err(err) = self
                .execute(TaskBody::new_prompt("What is 2 + 2?", model))
                .await
            {
                log::warn!("Model {} failed dummy request, ignoring it: {}", model, err);
                models_to_remove.push(model);
            }
        }

        // remove models that failed the dummy request
        for model in models_to_remove.iter() {
            models.remove(model);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires OpenRouter API key"]
    async fn test_openrouter_check() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_executor", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        let _ = dotenvy::dotenv(); // read api key

        let initial_models = [Model::OR3_5Sonnet, Model::OR3_7Sonnet];
        let mut models = HashSet::from_iter(initial_models);
        let config = OpenRouterClient::from_env().unwrap();
        config.check(&mut models).await;
        assert_eq!(models.len(), initial_models.len());

        // create with a bad api key
        let config = OpenRouterClient::new("i-dont-work");
        config.check(&mut HashSet::new()).await; // should not panic
    }
}
