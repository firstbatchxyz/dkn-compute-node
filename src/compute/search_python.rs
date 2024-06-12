use crate::{config::constants::*, utils::http::BaseClient};
use serde_json::json;
use std::env;

const DEFAULT_SEARCH_AGENT_URL: &str = "http://localhost:5059";

/// A wrapper for the Dria Search agent in Python: <https://github.com/firstbatchxyz/dria-searching-agent>.
pub struct SearchPythonClient {
    pub client: BaseClient,
    /// URL at which the Python search agent is running.
    pub url: String,
    /// Enables or disables manager, see more [here](https://docs.crewai.com/how-to/Hierarchical/).
    pub with_manager: bool,
}

impl Default for SearchPythonClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchPythonClient {
    pub fn new() -> Self {
        let url = env::var(SEARCH_AGENT_URL).unwrap_or(DEFAULT_SEARCH_AGENT_URL.to_string());
        let with_manager = matches!(
            env::var(SEARCH_AGENT_MANAGER)
                .unwrap_or_default()
                .to_lowercase()
                .as_str(),
            "1" | "true" | "yes"
        );

        let client = BaseClient::new(url.to_string());

        Self {
            client,
            url,
            with_manager,
        }
    }

    pub async fn search(&self, query: String) -> Result<String, reqwest::Error> {
        let body = json!({
            "query": query,
            "with_manager": self.with_manager,
        });
        let r = match self.client.post("search", body).await {
            Ok(response) => response,
            Err(e) => {
                log::error!("Error sending search query to search-agent-python: {}", e);
                return Err(e);
            }
        };

        let search_result = match r.text().await {
            Ok(response) => response,
            Err(e) => {
                log::error!("Error parsing search-agent-python response: {}", e);
                return Err(e);
            }
        };

        Ok(search_result)
    }
}
