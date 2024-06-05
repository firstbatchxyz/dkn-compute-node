use colored::Colorize;

#[path = "./common/ollama.rs"]
mod common;

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

    let (generation, duration) = common::use_model_with_prompt(model, prompt).await;
    println!(
        "\n{} ({}: {}ms): {}",
        "Response".green(),
        model,
        duration.as_millis(),
        generation.response
    );
    let tps = (generation.eval_count.unwrap_or_default() as f64)
        / (generation.eval_duration.unwrap_or(1) as f64)
        * 1_000_000_000f64;
    println!("{}: {}", "Tokens per second".yellow(), tps);
}
