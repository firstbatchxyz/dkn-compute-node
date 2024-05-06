use colored::Colorize;
use dkn_compute::compute::ollama::use_model_with_prompt;
use std::collections::HashMap;

/// This benchmark measures the time it takes to generate a response from a given Ollama model.
///
/// The provided response time is almost equivalent to the log generated within Ollama for `/api/generate` endpoint.
///
/// Note that the time it takes to evaluate the prompt and time it takes to generate the response is different, our computations
/// are based on the total time including both. This is to reflect the overall time of the single "computation task".
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

    let mut millis_per_char = HashMap::new();
    let num_prompts = prompts.len() as f64;
    for prompt in prompts {
        println!("{}: {}", "Prompt".blue(), prompt);
        for model in models {
            let (generation, duration) = use_model_with_prompt(model, prompt).await;
            println!(
                "\n{} ({}: {}ms): {}",
                "Response".green(),
                model,
                duration.as_millis(),
                generation.response
            );

            millis_per_char.insert(
                model,
                millis_per_char.get(model).unwrap_or(&0.0)
                    + ((generation.final_data.unwrap().eval_count as f64 / duration.as_secs_f64())
                        / num_prompts),
            );
        }

        println!("\n");
    }

    println!("Average {} for each model:", "tokens per second".yellow());
    for model in models {
        println!("{:<12}\t{}", model, millis_per_char.get(model).unwrap());
    }
}
