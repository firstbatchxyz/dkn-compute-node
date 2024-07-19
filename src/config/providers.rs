use ollama_rs::Ollama;
use std::env;

/// Checks for OpenAI API key.
pub fn check_openai() -> Result<(), String> {
    const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

    if env::var(OPENAI_API_KEY).is_err() {
        return Err("OpenAI API key not found".into());
    }

    Ok(())
}

/// Checks that Ollama is running, and required models are there.
pub async fn check_ollama(
    host: &str,
    port: u16,
    required_models: Vec<String>,
) -> Result<(), String> {
    let ollama = Ollama::new(host.trim_matches('"'), port);

    // fetch local models
    let local_models = match ollama.list_local_models().await {
        Ok(models) => models.into_iter().map(|m| m.name).collect::<Vec<_>>(),
        Err(e) => {
            return {
                log::error!("Could not fetch local models from Ollama, is it online?");
                Err(e.to_string())
            }
        }
    };

    // check that each required model exists here
    log::debug!("Checking required models: {:#?}", required_models);
    log::debug!("Found local models: {:#?}", local_models);
    for model in required_models {
        if !local_models.iter().any(|m| *m == model) {
            log::error!("Model {} not found in Ollama", model);
            log::error!("Please download it with: ollama pull {}", model);
            return Err("Required model not pulled in Ollama.".into());
        }
    }

    Ok(())
}
