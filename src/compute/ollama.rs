use std::env;

use ollama_rs::{
    error::OllamaError,
    generation::completion::{request::GenerationRequest, GenerationResponse},
    Ollama,
};
use tokio_util::sync::CancellationToken;

pub const DEFAULT_DKN_OLLAMA_HOST: &str = "http://127.0.0.1";
pub const DEFAULT_DKN_OLLAMA_PORT: u16 = 11434;
pub const DEFAULT_DKN_OLLAMA_MODEL: &str = "orca-mini";

/// A wrapper for the Ollama API.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub(crate) client: Ollama,
    pub(crate) model: String,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new(
            Some(DEFAULT_DKN_OLLAMA_HOST.to_string()),
            Some(DEFAULT_DKN_OLLAMA_PORT),
            Some(DEFAULT_DKN_OLLAMA_MODEL.to_string()),
        )
    }
}

impl OllamaClient {
    /// Creates a new Ollama client.
    ///
    /// Reads `DKN_OLLAMA_HOST`, `DKN_OLLAMA_PORT` and `DKN_OLLAMA_MODEL` from the environment, and defaults if not provided.
    pub fn new(host: Option<String>, port: Option<u16>, model: Option<String>) -> Self {
        let host = host.unwrap_or_else(|| {
            env::var("DKN_OLLAMA_HOST").unwrap_or(DEFAULT_DKN_OLLAMA_HOST.to_string())
        });

        let port = port.unwrap_or_else(|| {
            env::var("DKN_OLLAMA_PORT")
                .and_then(|port_str| {
                    port_str
                        .parse::<u16>()
                        .map_err(|_| env::VarError::NotPresent)
                })
                .unwrap_or(DEFAULT_DKN_OLLAMA_PORT)
        });

        let model = model.unwrap_or_else(|| {
            env::var("DKN_OLLAMA_MODEL").unwrap_or(DEFAULT_DKN_OLLAMA_MODEL.to_string())
        });

        let client = Ollama::new(host, port);
        log::info!("Ollama URL: {}", client.uri());
        log::info!("Ollama Model: {}", model);

        Self { client, model }
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
    pub async fn generate(&self, prompt: String) -> Result<GenerationResponse, OllamaError> {
        log::debug!("Generating with prompt: {}", prompt);

        let gen_req = GenerationRequest::new(self.model.clone(), prompt);
        let gen_res = self.client.generate(gen_req).await?;

        log::debug!("Generated response: {}", gen_res.response);
        Ok(gen_res)
    }
}

pub async fn use_model_with_prompt(
    model: &str,
    prompt: &str,
) -> (GenerationResponse, tokio::time::Duration) {
    use crate::utils::get_current_time_nanos;

    let ollama = OllamaClient::new(None, None, Some(model.to_string()));
    ollama
        .setup(CancellationToken::default())
        .await
        .expect("Should pull model");

    let time = get_current_time_nanos();
    let prompt = prompt.to_string();

    let gen_res = ollama
        .generate(prompt)
        .await
        .expect("Should generate response");
    let time_diff = get_current_time_nanos() - time;
    let duration = tokio::time::Duration::from_nanos(time_diff as u64);

    (gen_res, duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config() {
        env::set_var("DKN_OLLAMA_HOST", "im-a-host");
        env::set_var("DKN_OLLAMA_MODEL", "phi3");
        env::remove_var("DKN_OLLAMA_PORT");

        // will use default port, but read host and model from env
        let ollama = OllamaClient::new(None, None, None);
        assert_eq!(ollama.client.uri(), "im-a-host:11434");
        assert_eq!(ollama.model, "phi3");
    }
}
