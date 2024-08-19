#![allow(unused)]

use serde::Deserialize;

const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

const OPENAI_MODELS_API: &str = "https://api.openai.com/v1/models";

/// [Model](https://platform.openai.com/docs/api-reference/models/object) API object.
#[derive(Debug, Clone, Deserialize)]
struct OpenAIModel {
    /// The model identifier, which can be referenced in the API endpoints.
    id: String,
    /// The Unix timestamp (in seconds) when the model was created.
    created: u64,
    /// The object type, which is always "model".
    object: String,
    /// The organization that owns the model.
    owned_by: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
    object: String,
}

#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub(crate) api_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIError {
    message: String,
}

impl OpenAIConfig {
    /// Looks at the environment variables for OpenAI API key.
    pub fn new() -> Self {
        let api_key = std::env::var(OPENAI_API_KEY).ok();

        Self { api_key }
    }

    /// Check if requested models exist.
    pub async fn check(&self, models: Vec<String>) -> Result<(), String> {
        log::info!("Checking OpenAI requirements");

        let Some(api_key) = &self.api_key else {
            return Err("OpenAI API key not found".into());
        };

        let client = reqwest::Client::new();
        let request = client
            .get(OPENAI_MODELS_API)
            .header("Authorization", format!("Bearer {}", api_key))
            .build()
            .map_err(|e| format!("Failed to build request: {}", e))?;

        let response = client
            .execute(request)
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if response.status().is_client_error() {
            return Err(format!(
                "Failed to fetch OpenAI models:\n{}",
                response.text().await.unwrap_or_default()
            ));
        }

        let openai_models = response.json::<OpenAIModelsResponse>().await.unwrap();
        for requested_model in models {
            if !openai_models.data.iter().any(|m| m.id == requested_model) {
                return Err(format!(
                    "Model {} not found in your OpenAI account.",
                    requested_model
                ));
            }
        }

        log::info!("OpenAI setup is all good.",);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_openai_check() {
        let config = OpenAIConfig::new();
        let res = config.check(vec![]).await;
        println!("Result: {}", res.unwrap_err());
    }
}
