use std::env;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use langchain_rust::llm::client::Ollama as OllamaLang; // langchain Ollama client
use ollama_rs::Ollama; // langchain's Ollama-rs instance

pub const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
pub const DEFAULT_OLLAMA_PORT: u16 = 11434;
pub const DEFAULT_OLLAMA_MODEL: &str = "orca-mini";

/// Creates an Ollama client, pulls the model if it does not exist locally.
pub async fn create_ollama(cancellation: CancellationToken) -> Result<OllamaLang, String> {
    let host = env::var("OLLAMA_HOST").unwrap_or(DEFAULT_OLLAMA_HOST.to_string());

    let port = env::var("OLLAMA_PORT")
        .and_then(|port_str| {
            port_str
                .parse::<u16>()
                .map_err(|_| env::VarError::NotPresent)
        })
        .unwrap_or(DEFAULT_OLLAMA_PORT);

    let model = env::var("OLLAMA_MODEL").unwrap_or(DEFAULT_OLLAMA_MODEL.to_string());

    let client = Ollama::new(host, port);
    log::info!("Ollama URL: {}", client.uri());
    log::info!("Ollama Model: {}", model);

    pull_model(&client, &model, cancellation).await?;

    Ok(OllamaLang::new(Arc::new(client), model, None))
}

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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_ollama_config() {
//         env::set_var("OLLAMA_HOST", "http://im-a-host");
//         env::set_var("OLLAMA_MODEL", "phi3");
//         env::remove_var("OLLAMA_PORT");

//         // will use default port, but read host and model from env
//         let ollama = OllamaClient::new(None, None, None);
//         assert_eq!(ollama.client.url_str(), "http://im-a-host:11434/");
//         assert_eq!(ollama.model, "phi3");
//     }
// }
