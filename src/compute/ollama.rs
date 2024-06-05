use langchain_rust::language_models::llm::LLM;
use std::env;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use langchain_rust::llm::client::Ollama as OllamaLang; // langchain Ollama client
use ollama_rs::Ollama; // our Ollama-rs instance
use ollama_rs_old::Ollama as OllamaOld; // langchain's Ollama-rs instance

use ollama_rs::error::OllamaError;

pub const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
pub const DEFAULT_OLLAMA_PORT: u16 = 11434;
pub const DEFAULT_OLLAMA_MODEL: &str = "orca-mini";

/// A wrapper for the Ollama API.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub client: Ollama,
    pub(crate) langchain: OllamaLang,
    pub(crate) model: String,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new(
            Some(DEFAULT_OLLAMA_HOST.to_string()),
            Some(DEFAULT_OLLAMA_PORT),
            Some(DEFAULT_OLLAMA_MODEL.to_string()),
        )
    }
}

impl OllamaClient {
    /// Creates a new Ollama client.
    ///
    /// Reads `OLLAMA_HOST`, `OLLAMA_PORT` and `OLLAMA_MODEL` from the environment, and defaults if not provided.
    pub fn new(host: Option<String>, port: Option<u16>, model: Option<String>) -> Self {
        let host = host
            .unwrap_or_else(|| env::var("OLLAMA_HOST").unwrap_or(DEFAULT_OLLAMA_HOST.to_string()));

        let port = port.unwrap_or_else(|| {
            env::var("OLLAMA_PORT")
                .and_then(|port_str| {
                    port_str
                        .parse::<u16>()
                        .map_err(|_| env::VarError::NotPresent)
                })
                .unwrap_or(DEFAULT_OLLAMA_PORT)
        });

        let model = model.unwrap_or_else(|| {
            env::var("OLLAMA_MODEL").unwrap_or(DEFAULT_OLLAMA_MODEL.to_string())
        });

        let client = Ollama::new(host.clone(), port);
        log::info!("Ollama URL: {}", client.uri());
        log::info!("Ollama Model: {}", model);

        let client_old = OllamaOld::new(host, port);
        let langchain = OllamaLang::new(Arc::new(client_old), model.clone(), None);

        Self {
            langchain,
            client,
            model,
        }
    }

    /// Lists local models for diagnostic, and pulls the configured model.
    pub async fn setup(&self, cancellation: CancellationToken) -> Result<(), OllamaError> {
        log::info!("Checking local models");
        let local_models = self.client.list_local_models().await?;
        let num_local_modals = local_models.len();
        if num_local_modals == 0 {
            log::info!("No local models found.");
        } else {
            let mut message = format!("{}{}", num_local_modals, " local models found:");
            for model in local_models.iter() {
                message.push_str(format!("\n{}", model.name).as_str())
            }
            log::info!("{}", message);
        }

        log::info!("Pulling model: {}, this may take a while...", self.model);
        const MAX_RETRIES: usize = 3;
        let mut retry_count = 0; // retry count for edge case
        while let Err(e) = self.client.pull_model((&self.model).into(), false).await {
            // edge case: invalid model is given
            if e.to_string().contains("file does not exist") {
                return Err(OllamaError::from(
                    "Invalid Ollama model, please check your environment variables.".to_string(),
                ));
            } else if retry_count < MAX_RETRIES {
                log::error!(
                    "Error setting up Ollama: {}\nRetrying in 5 seconds ({}/{}).",
                    e,
                    retry_count,
                    MAX_RETRIES
                );
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                        retry_count += 1; // Increment the retry counter
                        continue;
                    }
                }
            } else {
                // Handling the case when maximum retries are exceeded
                log::error!("Maximum retry attempts exceeded, stopping retries.");
                return Err(OllamaError::from(
                    "Maximum retry attempts exceeded.".to_string(),
                ));
            }
        }
        log::info!("Pulled {}", self.model);

        Ok(())
    }

    /// Generates a result using the local LLM.
    pub async fn generate(&self, prompt: String) -> Result<String, OllamaError> {
        log::debug!("Generating with prompt: {}", prompt);

        let response = self
            .langchain
            .invoke(&prompt)
            .await
            .map_err(|e| OllamaError::from(format!("{:?}", e)))?;

        log::debug!("Generated response: {}", response);
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config() {
        env::set_var("OLLAMA_HOST", "http://im-a-host");
        env::set_var("OLLAMA_MODEL", "phi3");
        env::remove_var("OLLAMA_PORT");

        // will use default port, but read host and model from env
        let ollama = OllamaClient::new(None, None, None);
        assert_eq!(ollama.client.url_str(), "http://im-a-host:11434/");
        assert_eq!(ollama.model, "phi3");
    }
}
