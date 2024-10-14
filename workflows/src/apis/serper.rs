use eyre::{eyre, Context, Result};
use reqwest::Client;
use std::env;

use crate::utils::safe_read_env;

/// Makes a search request.
const SERPER_EXAMPLE_ENDPOINT: &str = "https://google.serper.dev/search";
const ENV_VAR_NAME: &str = "SERPER_API_KEY";

/// Serper-specific configurations.
#[derive(Debug, Clone, Default)]
pub struct SerperConfig {
    /// API key, if available.
    api_key: Option<String>,
}

impl SerperConfig {
    /// Looks at the environment variables for Serper API key.
    pub fn new() -> Self {
        Self {
            api_key: safe_read_env(env::var(ENV_VAR_NAME)),
        }
    }

    /// Sets the API key for Serper.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Check if Serper API KEY exists and if it does, tries a dummy request.
    /// Fails if the provided API KEY is not authorized enough for the dummy request.
    ///
    /// Equivalent cURL is as follows:
    ///
    /// ```sh
    /// curl -X POST 'https://google.serper.dev/search' \
    /// -H 'X-API-KEY: API_KEY' \
    /// -H 'Content-Type: application/json' \
    /// -d '{
    ///  "q": "Your search query here"
    /// }'
    /// ```
    pub async fn check_optional(&self) -> Result<()> {
        // check API key
        let Some(api_key) = &self.api_key else {
            log::debug!("Serper API key not found, skipping Serper check");
            return Ok(());
        };
        println!("API KEY: {}", api_key);
        log::info!("Serper API key found, checking Serper service");

        // make a dummy request
        let client = Client::new();
        let request = client
            .post(SERPER_EXAMPLE_ENDPOINT)
            .header("X-API-KEY", api_key)
            .header("Content-Type", "application/json")
            .body("{\"q\": \"Your search query here\"}")
            .build()
            .wrap_err("failed to build request")?;

        let response = client
            .execute(request)
            .await
            .wrap_err("failed to send request")?;

        // parse response
        if response.status().is_client_error() {
            return Err(eyre!("Failed to make Serper request",))
                .wrap_err(response.text().await.unwrap_or_default());
        }

        log::info!("Serper check succesful!");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Serper API key"]
    async fn test_serper_check() {
        let _ = dotenvy::dotenv();
        assert!(env::var(ENV_VAR_NAME).is_ok());
        let res = SerperConfig::new().check_optional().await;
        assert!(res.is_ok(), "should pass with api key");

        env::set_var(ENV_VAR_NAME, "i-dont-work");
        let res = SerperConfig::new().check_optional().await;
        assert!(res.is_err(), "should fail with bad api key");

        env::remove_var(ENV_VAR_NAME);
        let res = SerperConfig::new().check_optional().await;
        assert!(res.is_ok(), "should pass without api key");
    }
}
