use eyre::Result;
use rig::completion::{Chat, PromptError};
use rig::providers::openrouter;

use crate::{Model, TaskBody};

/// OpenRouter-specific configurations.
#[derive(Clone)]
pub struct OpenRouterProvider {
    client: openrouter::Client,
}

impl OpenRouterProvider {
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
    pub async fn check(&self, external_models: Vec<Model>) -> Vec<Model> {
        log::info!("Checking OpenRouter API key");

        // make a dummy request with existing models
        let mut available_models = Vec::new();
        for requested_model in external_models {
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

            available_models.push(requested_model)
        }

        // log results
        if available_models.is_empty() {
            log::warn!("OpenRouter checks are finished, no available models found.",);
        } else {
            log::info!(
                "OpenRouter checks are finished, using models: {:#?}",
                available_models
            );
        }

        available_models
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
            .filter_module("dkn_workflows", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        let _ = dotenvy::dotenv(); // read api key

        let models = vec![Model::ORDeepSeek2_5, Model::ORLlama3_1_8B];
        let config = OpenRouterProvider::from_env().unwrap();
        let res = config.check(models.clone()).await;
        assert_eq!(res, models);

        // create with a bad api key
        let config = OpenRouterProvider::new("i-dont-work");
        let res = config.check(vec![]).await;
        assert!(res.is_empty()); // does not return an Err unlike others!
    }
}
