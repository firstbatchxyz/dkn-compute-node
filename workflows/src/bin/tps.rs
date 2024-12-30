#[cfg(not(feature = "profiling"))]
fn main() {
    unimplemented!("this binary requires the 'profiling' feature to be enabled");
}

#[cfg(feature = "profiling")]
#[tokio::main]
async fn main() {
    use dkn_workflows::{DriaWorkflowsConfig, OllamaConfig};
    use ollama_workflows::ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
    use ollama_workflows::Model;
    use prettytable::{Cell, Row, Table};
    use sysinfo::{CpuRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL};

    // initialize logger
    env_logger::init();

    let models = vec![
        Model::NousTheta,
        Model::Phi3Medium,
        Model::Phi3Medium128k,
        Model::Phi3_5Mini,
        Model::Phi3_5MiniFp16,
        Model::Gemma2_9B,
        Model::Gemma2_9BFp16,
        Model::Llama3_1_8B,
        Model::Llama3_1_8Bq8,
        Model::Llama3_1_8Bf16,
        Model::Llama3_1_8BTextQ4KM,
        Model::Llama3_1_8BTextQ8,
        Model::Llama3_1_70B,
        Model::Llama3_1_70Bq8,
        Model::Llama3_1_70BTextQ4KM,
        Model::Llama3_2_1B,
        Model::Llama3_2_3B,
        Model::Llama3_2_1BTextQ4KM,
        Model::Qwen2_5_7B,
        Model::Qwen2_5_7Bf16,
        Model::Qwen2_5_32Bf16,
        Model::Qwen2_5Coder1_5B,
        Model::Qwen2_5coder7B,
        Model::Qwen2_5oder7Bq8,
        Model::Qwen2_5coder7Bf16,
        Model::DeepSeekCoder6_7B,
        Model::Mixtral8_7b,
        Model::GPT4Turbo,
        Model::GPT4o,
        Model::GPT4oMini,
        Model::O1Preview,
        Model::O1Mini,
        Model::Gemini15ProExp0827,
        Model::Gemini15Pro,
        Model::Gemini15Flash,
        Model::Gemini10Pro,
        Model::Gemma2_2bIt,
        Model::Gemma2_27bIt,
    ];

    let cfg = DriaWorkflowsConfig::new(models);
    let config = OllamaConfig::default();
    let ollama = Ollama::new(config.host, config.port);
    log::debug!("Starting...");
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
    let mut tps: f64;
    let mut table = Table::new();

    // Add a row with the headers
    table.add_row(Row::new(vec![
        Cell::new("Model"),
        Cell::new("TPS"),
        Cell::new("OS"),
        Cell::new("Version"),
        Cell::new("CPU Usage (%)"),
        Cell::new("Total Memory (KB)"),
        Cell::new("Used Memory (KB)"),
    ]));

    for (_, model) in cfg.models {
        log::debug!("Pulling model: {}", model);

        // pull model
        match ollama.pull_model(model.to_string(), false).await {
            Ok(status) => log::debug!("Status: {}", status.message),
            Err(err) => {
                log::error!("Failed to pull model {}: {:?}", model, err);
            }
        }

        log::debug!("Creating request...");
        // create dummy request as a warm-up
        let generation_request =
            GenerationRequest::new(model.to_string(), "compute 6780 * 1200".to_string());

        // generate response
        match ollama.generate(generation_request).await {
            Ok(response) => {
                log::debug!("Got response for model {}", model);

                // compute TPS
                tps = (response.eval_count.unwrap_or_default() as f64)
                    / (response.eval_duration.unwrap_or(1) as f64)
                    * 1_000_000_000f64;

                // add row to table
                // FIXME: this should be updated on each iteration
                table.add_row(Row::new(vec![
                    Cell::new(&model.to_string()),
                    Cell::new(&tps.to_string()),
                    Cell::new(&format!("{} {}", brand, os_name)),
                    Cell::new(&os_version),
                    Cell::new(&cpu_usage.to_string()),
                    Cell::new(&total_memory.to_string()),
                    Cell::new(&used_memory.to_string()),
                ]));
            }
            Err(e) => {
                log::warn!("Ignoring model {}: Workflow failed with error {}", model, e);
            }
        }
        table.printstd();
        // print system info
        // refresh CPU usage (https://docs.rs/sysinfo/latest/sysinfo/struct.Cpu.html#method.cpu_usage)
        system =
            System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
        // wait a bit because CPU usage is based on diff
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        // refresh CPUs again to get actual value
        system.refresh_cpu_usage();
    }
    // print system info
    table.printstd();
    log::debug!("Finished");
}
