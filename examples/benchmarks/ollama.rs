use colored::Colorize;
use dkn_compute::compute::ollama::use_model_with_prompt; 
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::Read; 

/// A `println!` macro that only prints when the `debug_assertions` flag is set, i.e. it wont print when `--release` is used.
macro_rules! debug_println {
    ($($arg:tt)*) => (if ::std::cfg!(debug_assertions) { ::std::println!($($arg)*); })
}

/// Shareable format string to print results.
macro_rules! result_format_str {
    () => {
        "{:<15} {:<18} {:<18} {:<18} {:<18} {:<18} {:<18}"
    };
}

#[tokio::main]
async fn main() {
    let models = ["orca-mini"]; //, "phi3", "llama3", "openhermes"];
    let preset_prompts =  [  
        "Give 3 names of famous scientists, 1 Field Medalist, 1 Turing Award recipient and 1 Nobel laureate. Provide only the names, such as: 1. John Doe, 2. Jane Doe, 3. Foo Bar.",
    ];

    // decide on prompts to be used
    let prompts: Vec<String> = match env::var("JSON_PATH") {
        Ok(path) => {
            println!("Reading tasks from: {}", path);
            let jobs = read_json_file::<Vec<Job>>(path.as_str()).unwrap();
            jobs.into_iter().map(|job| job.prompt).collect()
        },
        Err(_) => {
            println!("Using preset prompts.");
            preset_prompts.iter().map(|&prompt| prompt.to_string()).collect()
        }
    }; 

    // let mut tokens_per_second = HashMap::new();
    let mut results = Vec::new();
    // let num_prompts = prompts.len() as f64;
    
    for (prompt_num, prompt) in prompts.iter().enumerate(){ 
        // println!("{}{}: {}", "Prompt #".blue(), prompt_num, prompt);
        print_title();

        for model in models {
            // will loop until it can generate a result with "final data"
            loop {
                let (generation, duration) = use_model_with_prompt(model, prompt).await;
             
                if let Some(gen_data) = generation.final_data {
                    let result = BenchmarkResult {  
                        prompt_num,
                        model: model.to_string(),
                        api_duration: duration.as_nanos(),
                        total_duration: gen_data.total_duration,
                        prompt_eval_count: gen_data.prompt_eval_count,
                        prompt_eval_duration: gen_data.prompt_eval_duration,
                        eval_count: gen_data.eval_count,
                        eval_duration: gen_data.eval_duration,
                    };
                    println!("{}", result);
                    results.push(result);
                    break;
                } else { 
                    println!("{}: {}", "Warn".yellow(), "Could not get final data.");
                }
            }
            
        }
    }

       // tokens_per_second.insert(
                    //     model,
                    //     tokens_per_second.get(model).unwrap_or(&0.0)
                    //         + ((gen_data.eval_count as f64 / (gen_data.total_duration as f64 / 1_000_000_000f64))
                    //             / num_prompts),
                    // );
                    
    // println!("Average {} for each model:", "tokens per second".yellow());
    // for model in models {
    //     println!("{:<12}\t{}", model, tokens_per_second.get(model).unwrap());
    // }
}

/// Reads a JSON file and deserializes it.
fn read_json_file<T: for<'a> Deserialize<'a>>(file_path: &str) -> Result<T, std::io::Error> {
    let mut file = File::open(file_path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let obj = serde_json::from_str(&contents)?;
    Ok(obj)
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            result_format_str!(),
            self.model,
            self.api_duration,
            self.total_duration,
            self.prompt_eval_count,
            self.prompt_eval_duration,
            self.eval_count,
            self.eval_duration,
        )
    }
}

#[inline(always)]
fn print_title() {
    println!(
        result_format_str!(),
        "Model".blue(),
        "Call (ns)".cyan(),
        "Total (ns)".red(),
        "Prompt (t)".yellow(),
        "Prompt (ns)".yellow(),
        "Result (t)".green(),
        "Result (ns)".green(),
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkResult {
    /// Prompt number
    pub prompt_num: usize,
    /// Model used to generate the result
    pub model: String, 
    /// Time spent making the entire API call to Ollama
    pub api_duration: u128,
    /// Time spent evaluating the prompt & generating the response
    pub total_duration: u64,
    /// Number of tokens in the prompt
    pub prompt_eval_count: u16,
    /// Time spent in nanoseconds evaluating the prompt
    pub prompt_eval_duration: u64,
    /// Number of tokens in the response
    pub eval_count: u16,
    /// Time in nanoseconds spent generating the response
    pub eval_duration: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Job {
    purpose: String,
    task_type: String,
    dataset_format: String,
    language: String,
    alignment_preferences: Vec<String>,
    dataset_size: usize,
    id: String,
    private_key: String,
    public_key: String,
    status: String,
    prompt: String,
    subtask_id: String,
}
