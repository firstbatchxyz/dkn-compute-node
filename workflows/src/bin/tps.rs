use dkn_workflows::{DriaWorkflowsConfig, OllamaConfig};
use ollama_workflows::ollama_rs::{
    generation::{completion::request::GenerationRequest, options::GenerationOptions},
    Ollama,
};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[tokio::main]
async fn main() {
    // initialize logger
    env_logger::init();

    let cfg = DriaWorkflowsConfig::new_from_csv("finalend/hermes-3-llama-3.1:8b-q8_0,phi3:14b-medium-4k-instruct-q4_1,phi3:14b-medium-128k-instruct-q4_1,phi3.5:3.8b,phi3.5:3.8b-mini-instruct-fp16,gemma2:9b-instruct-q8_0,gemma2:9b-instruct-fp16,llama3.1:latest,llama3.1:8b-instruct-q8_0,llama3.1:8b-instruct-fp16,llama3.1:70b-instruct-q4_0,llama3.1:70b-instruct-q8_0,llama3.2:1b,llama3.2:3b,qwen2.5:7b-instruct-q5_0,qwen2.5:7b-instruct-fp16,qwen2.5:32b-instruct-fp16,qwen2.5-coder:1.5b,qwen2.5-coder:7b-instruct,llama3.2:3b,qwen2.5-coder:7b-instruct-q8_0,qwen2.5-coder:7b-instruct-fp16,deepseek-coder:6.7b,mixtral:8x7b");
    let config = OllamaConfig::default();
    let ollama = Ollama::new(config.host, config.port);

    log::info!("Starting...");
    // ensure that all lists of CPUs and processes are filled
    let mut system = System::new_all();
    // update all information of the system
    system.refresh_all();

    log::debug!("Getting system information...");
    let brand = system.cpus()[0].brand().to_string();
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
    let cpu_usage = system.global_cpu_usage();
    let total_memory = system.total_memory();
    let used_memory = system.used_memory();

    for (_, model) in cfg.models {
        log::info!("Pulling model: {}", model);

        // pull model
        match ollama.pull_model(model.to_string(), false).await {
            Ok(status) => log::info!("Status: {}", status.message),
            Err(err) => {
                log::error!("Failed to pull model {}: {:?}", model, err);
            }
        }

        log::debug!("Creating request...");
        // create dummy request
        let mut generation_request =
            GenerationRequest::new(model.to_string(), "compute 6780 * 1200".to_string());

        if let Ok(num_thread) = std::env::var("OLLAMA_NUM_THREAD") {
            generation_request = generation_request.options(
                GenerationOptions::default().num_thread(
                    num_thread
                        .parse()
                        .expect("num threads should be a positive integer"),
                ),
            );
        }

        // generate response
        match ollama.generate(generation_request).await {
            Ok(response) => {
                log::debug!("Got response for model {}", model);
                // compute TPS
                let tps = (response.eval_count.unwrap_or_default() as f64)
                    / (response.eval_duration.unwrap_or(1) as f64)
                    * 1_000_000_000f64;
                // report machine info
                log::info!(
                    "\n Model: {} \n TPS: {} \n OS: {} {} \n Version: {} \n CPU Usage: % {} \n Total Memory: {} KB \n Used Memory: {} KB ",
                    model,
                    tps,
                    brand,
                    os_name,
                    os_version,
                    cpu_usage,
                    total_memory,
                    used_memory,
                );
            }
            Err(e) => {
                log::warn!("Ignoring model {}: Workflow failed with error {}", model, e);
            }
        }
        // refresh CPU usage (https://docs.rs/sysinfo/latest/sysinfo/struct.Cpu.html#method.cpu_usage)
        system =
            System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
        // wait a bit because CPU usage is based on diff
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        // refresh CPUs again to get actual value
        system.refresh_cpu_usage();
    }
    log::info!("Finished");
}
