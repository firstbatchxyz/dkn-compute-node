use colored::Colorize;
use dkn_compute::{compute::ollama::OllamaClient, utils::get_current_time_nanos};
use ollama_rs::generation::completion::GenerationResponse;
use std::time::Duration;

/// This benchmark measures the time it takes to generate a response from a given Ollama model.
///
/// The provided response time is almost equivalent to the log generated within Ollama for `/api/generate` endpoint.
///
/// Note that the time it takes to evaluate the prompt and time it takes to generate the response is different, our computations
/// are based on the total time including both. This is to reflect the overall time of the single "computation task".
#[tokio::main]
async fn main() {
    let model = "phi3";

    let prompt =  "You will be generating a synthetic dataset based on the following information:

<Task Type>
Fine-tuning
</Task Type>

<Dataset Format>
The dataset will consist of instructions to generate jokes, followed by the corresponding joke output.
</Dataset Format>


<Alignment Preference>
Include a wide range of jokes, as long as they are funny.
Do not exclude sensitive or insensitive jokes.
</Alignment Prompt>

<Ground Truth Examples>
['Humor is driven by culture and there are stereotypes about humor among different cultures.', 'There are four different types of humor styles: affiliative, self-enhancing, self-defeating, and aggressive.', 'Comedy Trade School offers a stand-up comedy course taught by professional comedians', 'There are age and cultural differences in humor appreciation, with different groups endorsing some humor styles more than others.', 'Watching the video and taking the quiz can help improve understanding of joke structure']
</Ground Truth Examples>

Please carefully analyze the preference prompt to understand the desired characteristics, structure, and content of the dataset you will be generating. Pay close attention to any specific requirements or constraints mentioned in the prompt.

Use the provided ground truth examples as a reference for the format, style, and quality of the examples you will generate. Ensure that your generated examples are consistent with the patterns and characteristics observed in the ground truth examples.

Generate 5 synthetic examples that align with the preference prompt and maintain consistency with the ground truth examples. Each example should be unique and diverse while still adhering to the specified requirements.

Please ensure that your generated examples are realistic, coherent, and free of any errors or inconsistencies. If the preference prompt specifies any additional constraints or considerations, make sure to incorporate them into your generated examples.

<Output>
[<ENTRY>, <ENTRY>, ...]
</Output


Output must be use only JSON-formatted synthetic datasets.
Do not include instruction, only entry. Do not add any comment.";

    let (generation, duration) = use_model_with_prompt(model, prompt).await;
    println!(
        "\n{} ({}: {}ms): {}",
        "Response".green(),
        model,
        duration.as_millis(),
        generation.response
    );
    let tps = generation.final_data.unwrap().eval_count as f64 / duration.as_secs_f64();
    println!("{}: {}", "Tokens per second".yellow(), tps);
}

async fn use_model_with_prompt(model: &str, prompt: &str) -> (GenerationResponse, Duration) {
    let ollama = OllamaClient::new(None, None, Some(model.to_string()));
    ollama.setup().await.expect("Should pull model");

    let time = get_current_time_nanos();
    let gen_res = ollama
        .generate(prompt.to_string())
        .await
        .expect("Should generate response");
    let time_diff = get_current_time_nanos() - time;
    let duration = Duration::from_nanos(time_diff as u64);

    (gen_res, duration)
}
