use std::time::SystemTime;

use ollama_rs::{
    generation::completion::{request::GenerationRequest, GenerationResponse},
    Ollama,
};

/// A shorthand function to invoke a prompt using a given model with Ollama.
pub async fn use_model_with_prompt(
    model: &str,
    prompt: &str,
) -> (GenerationResponse, tokio::time::Duration) {
    let ollama = Ollama::default();

    let time = get_current_time_nanos();
    let prompt = prompt.to_string();

    log::debug!("Generating with prompt: {}", prompt);

    let gen_req = GenerationRequest::new(model.to_string(), prompt);
    let gen_res = ollama.generate(gen_req).await.expect("should generate");

    log::debug!("Generated response: {}", gen_res.response);

    let time_diff = get_current_time_nanos() - time;
    let duration = tokio::time::Duration::from_nanos(time_diff as u64);

    (gen_res, duration)
}

#[inline(always)]
fn get_current_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
