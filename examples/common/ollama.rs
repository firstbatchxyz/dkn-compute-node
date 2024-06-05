use dkn_compute::{compute::ollama::OllamaClient, utils::get_current_time_nanos};
use ollama_rs::generation::completion::{request::GenerationRequest, GenerationResponse};
use tokio_util::sync::CancellationToken;

/// A shorthand function to invoke a prompt using a given model with Ollama.
pub async fn use_model_with_prompt(
    model: &str,
    prompt: &str,
) -> (GenerationResponse, tokio::time::Duration) {
    let ollama = OllamaClient::new(None, None, Some(model.to_string()));
    ollama
        .setup(CancellationToken::default())
        .await
        .expect("Should pull model");

    let time = get_current_time_nanos();
    let prompt = prompt.to_string();

    log::debug!("Generating with prompt: {}", prompt);

    let gen_req = GenerationRequest::new(model.to_string(), prompt);
    let gen_res = ollama
        .client
        .generate(gen_req)
        .await
        .expect("should generate");

    log::debug!("Generated response: {}", gen_res.response);

    let time_diff = get_current_time_nanos() - time;
    let duration = tokio::time::Duration::from_nanos(time_diff as u64);

    (gen_res, duration)
}
