use eyre::{eyre, Context, Result};
use ollama_workflows::Model;
use serde::Deserialize;

const OPENAI_MODELS_API: &str = "https://api.openai.com/v1/models";

/// [Model](https://platform.openai.com/docs/api-reference/models/object) API object.
#[derive(Debug, Clone, Deserialize)]
struct OpenAIModel {
    /// The model identifier, which can be referenced in the API endpoints.
    id: String,
    /// The Unix timestamp (in seconds) when the model was created.
    #[allow(unused)]
    created: u64,
    /// The object type, which is always "model".
    #[allow(unused)]
    object: String,
    /// The organization that owns the model.
    #[allow(unused)]
    owned_by: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
    #[allow(unused)]
    object: String,
}

#[derive(Debug, Clone, Default)]
pub struct OpenAIConfig {
    /// List of external models that are picked by the user.
    pub(crate) models: Vec<Model>,
}

impl OpenAIConfig {
    /// Looks at the environment variables for OpenAI API key.
    pub fn new() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY").ok();

        Self { api_key }
    }

    /// Check if requested models exist & are available in the OpenAI account.
    pub async fn check(&self, models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!("Checking OpenAI requirements");

        // check API key
        let Some(api_key) = &self.api_key else {
            return Err(eyre!("OpenAI API key not found"));
        };

        // fetch models
        let client = reqwest::Client::new();
        let request = client
            .get(OPENAI_MODELS_API)
            .header("Authorization", format!("Bearer {}", api_key))
            .build()
            .wrap_err("Failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("Failed to send request")?;

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
        let config = OpenAIConfig::new();
        let res = config.check(vec![]).await;
        println!("Result: {}", res.unwrap_err());
    }
}
