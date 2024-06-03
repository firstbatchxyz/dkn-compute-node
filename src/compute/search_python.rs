use crate::utils::http::BaseClient;
use serde_json::json;
use std::env;

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
        let url = env::var("SEARCH_AGENT_URL").unwrap_or_default();
        let with_manager = match env::var("SEARCH_AGENT_MANAGER")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "1" | "true" | "yes" => true,
            _ => false,
        };

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
                eprintln!("Error sending search query to search-agent-python: {:?}", e);
                return Err(e);
            }
        };

        let search_result = match r.text().await {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Error parsing search-agent-python response: {:?}", e);
                return Err(e);
            }
        };

        Ok(search_result)
    }
}
