use reqwest::Client;
use std::collections::HashMap;

// TODO: this may not be used atm

/// A wrapper for GET, POST and DELETE requests.
#[derive(Debug, Clone)]
pub struct BaseClient {
    base_url: String,
    client: Client,
}

impl BaseClient {
    pub fn new(url: String) -> Self {
        let client = Client::new();
        BaseClient {
            base_url: url,
            client,
        }
    }

    /// A generic GET request.
    pub async fn get(
        &self,
        url: &str,
        query_params: Option<HashMap<String, String>>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let mut full_url = format!("{}/{}", self.base_url, url);

        // add query parameters
        if let Some(params) = query_params {
            let query_string = convert_to_query_params(params);
            full_url.push_str(&format!("?{}", query_string));
        }

        let res = self
            .client
            .get(&full_url)
            .header("Accept", "application/json, text/plain")
            .send()
            .await?;

        res.error_for_status()
    }

    /// A generic POST request.
    pub async fn post(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let full_url = format!("{}/{}", self.base_url, url);

        let res = self
            .client
            .post(&full_url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        res.error_for_status()
    }

    /// A generic DELETE request.
    pub async fn delete(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let full_url = format!("{}/{}", self.base_url, url);

        let res = self
            .client
            .delete(&full_url)
            .header("Accept", "application/json, text/plain")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        res.error_for_status()
    }

    pub fn get_base_url(&self) -> String {
        self.base_url.clone()
    }
}

#[inline]
fn convert_to_query_params(params: HashMap<String, String>) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_query_params() {
        let mut params = HashMap::new();
        params.insert("key1".to_string(), "v_a lue/1".to_string());
        // we could test with multiple keys as well, but the ordering of parameters
        // may change sometimes which causes the test to fail randomly

        let expected = "key1=v_a+lue%2F1".to_string();
        assert_eq!(convert_to_query_params(params), expected);
    }
}
