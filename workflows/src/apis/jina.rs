use dkn_utils::safe_read_env;
use eyre::{eyre, Context, Result};
use reqwest::Client;
use std::env;

const ENV_VAR_NAME: &str = "JINA_API_KEY";

/// Jina-specific configurations.
#[derive(Debug, Clone, Default)]
pub struct JinaConfig {
    /// API key, if available.
    api_key: Option<String>,
}

impl JinaConfig {
    /// Looks at the environment variables for Jina API key.
    pub fn new() -> Self {
        Self {
            api_key: safe_read_env(env::var(ENV_VAR_NAME)),
        }
    }

    /// Checks if the API key is present.
    #[inline]
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Sets the API key for Jina.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Checks API KEY, and if it exists tries a dummy request.
    /// Fails if the provided API KEY is not authorized enough for the dummy request.
    ///
    /// Equivalent cURL is as follows:
    ///
    /// ```sh
    /// curl 'https://r.jina.ai/https://example.com' \
    /// -H "Authorization: Bearer jina_key"
    /// ```
    pub async fn check_optional(&self) -> Result<()> {
        // check API key
        let Some(api_key) = &self.api_key else {
            log::info!("Jina API key not found, skipping");
            return Ok(());
        };
        log::info!("Jina API key found, checking service");

        // make a dummy request to "example.com"
        let client = Client::new();
        let request = client
            .get("https://r.jina.ai/https://example.com")
            .header("Authorization", format!("Bearer {}", api_key))
            .build()
            .wrap_err("failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("failed to send request")?;

        // parse response
        if response.status().is_client_error() {
            return Err(eyre!("Failed to make Jina request",))
                .wrap_err(response.text().await.unwrap_or_default());
        }

        log::info!("Jina check succesful!");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Jina API key"]
    async fn test_jina_check() {
        let _ = dotenvy::dotenv();
        assert!(env::var(ENV_VAR_NAME).is_ok(), "should have api key");

        let res = JinaConfig::new().check_optional().await;
        assert!(res.is_ok(), "should pass with api key");

        env::set_var(ENV_VAR_NAME, "i-dont-work");
        let res = JinaConfig::new().check_optional().await;
        assert!(res.is_err(), "should fail with bad api key");

        env::remove_var(ENV_VAR_NAME);
        let res = JinaConfig::new().check_optional().await;
        assert!(res.is_ok(), "should pass without api key");
    }
}
