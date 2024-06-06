use std::env;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use langchain_rust::llm::client::Ollama as OllamaLang;
use ollama_rs::Ollama;

use crate::config::constants::*;

/// Creates an Ollama LangChain client, pulls the model if it does not exist locally.
pub async fn create_ollama(
    cancellation: CancellationToken,
    model: String,
) -> Result<OllamaLang, String> {
    let client = create_ollama_client();
    log::info!("Ollama URL: {}", client.uri());
    log::info!("Ollama Model: {}", model);

    pull_model(&client, &model, cancellation).await?;

    Ok(OllamaLang::new(Arc::new(client), model, None))
}

/// Creates the underlying OllamaRS client.
fn create_ollama_client() -> Ollama {
    let host = env::var(OLLAMA_HOST).unwrap_or(DEFAULT_OLLAMA_HOST.to_string());

    let port = env::var(OLLAMA_PORT)
        .and_then(|port_str| {
            port_str
                .parse::<u16>()
                .map_err(|_| env::VarError::NotPresent)
        })
        .unwrap_or(DEFAULT_OLLAMA_PORT);

    Ollama::new(host, port)
}

/// Pulls an LLM if it does not exist locally.
/// Also prints the locally installed models.
pub async fn pull_model(
    client: &Ollama,
    model: &str,
    cancellation: CancellationToken,
) -> Result<(), String> {
    log::info!("Checking local models");
    let local_models = client
        .list_local_models()
        .await
        .map_err(|e| format!("{:?}", e))?;

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

    log::info!("Pulling model: {}, this may take a while...", model);
    const MAX_RETRIES: usize = 3;
    let mut retry_count = 0; // retry count for edge case
    while let Err(e) = client.pull_model(model.to_string(), false).await {
        // edge case: invalid model is given
        if e.to_string().contains("file does not exist") {
            return Err(
                "Invalid Ollama model, please check your environment variables.".to_string(),
            );
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
            return Err("Maximum retry attempts exceeded.".to_string());
        }
    }
    log::info!("Pulled {}", model);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config() {
        env::set_var(OLLAMA_HOST, "http://im-a-host");
        env::remove_var(OLLAMA_PORT);

        // will use default port, but read host and model from env
        let ollama = create_ollama_client();
        assert_eq!(ollama.uri(), "http://im-a-host:11434");
    }
}
