use colored::Colorize;
use dkn_compute::{compute::ollama::OllamaClient, utils::get_current_time_nanos};
use std::time::Duration;

/// This benchmark measures the time it takes to generate a response from a given Ollama model.
///
/// The provided response time is almost equivalent to the log generated within Ollama for `/api/generate` endpoint.
#[tokio::main]
async fn main() {
    let models = ["orca-mini", "phi3", "llama3", "openhermes"];

    let prompts =  [
        "The sky appears blue during the day because of a process called scattering. \
    When sunlight enters the Earth's atmosphere, it collides with air molecules such as oxygen and nitrogen. \
    These collisions cause some of the light to be absorbed or reflected, which makes the colors we see appear more vivid and vibrant. \
    Blue is one of the brightest colors that is scattered the most by the atmosphere, making it visible to our eyes during the day. \
    What may be the question this answer? Be concise, provide at most 1-2 sentences.",

    "Give 3 names of famous scientists, 1 Field Medalist, 1 Turing Award recipient and 1 Nobel laureate. Provide only the names, such as: \
    1. John Doe, 2. Jane Doe, 3. Foo Bar.",
    ];

    for prompt in prompts {
        println!("{}: {}", "Prompt".blue(), prompt);
        for model in models {
            use_model_with_prompt(model, prompt).await;
        }

        println!("\n");
    }
}

async fn use_model_with_prompt(model: &str, prompt: &str) {
    let ollama = OllamaClient::new(None, None, Some(model.to_string()));
    ollama.setup().await.expect("Should pull model");

    let time = get_current_time_nanos();
    let gen_res = ollama
        .generate(prompt.to_string())
        .await
        .expect("Should generate response");
    let time_diff = get_current_time_nanos() - time;
    let duration = Duration::from_nanos(time_diff as u64);

    println!(
        "\n{} ({}: {}ms): {}",
        "Response".green(),
        model,
        duration.as_millis(),
        gen_res.response
    );
}
