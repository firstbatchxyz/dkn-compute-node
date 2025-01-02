use dkn_workflows::OllamaConfig;
use ollama_workflows::ollama_rs::{
    generation::completion::{request::GenerationRequest, GenerationResponse},
    Ollama,
};
use ollama_workflows::Model;

#[cfg(not(feature = "profiling"))]
fn main() {
    unimplemented!("this binary requires the 'profiling' feature to be enabled");
}

#[cfg(feature = "profiling")]
#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .filter_module("tps", log::LevelFilter::Info)
        .filter_module("dkn_workflows", log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let models = vec![
        Model::NousTheta,
        Model::Phi3Medium,
        Model::Phi3Medium128k,
        Model::Phi3_5Mini,
        Model::Phi3_5MiniFp16,
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
        Model::Gemma2_9B,
        Model::Gemma2_9BFp16,
    ];

    let config = OllamaConfig::default();
    let ollama = Ollama::new(config.host, config.port);

    run_benchmark(ollama, models).await;
}

#[cfg(feature = "profiling")]
async fn run_benchmark(ollama: Ollama, models: Vec<Model>) {
    use dkn_workflows::ModelProvider;
    use prettytable::{Cell, Row, Table};
    use sysinfo::{
        CpuRefreshKind, MemoryRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL,
    };

    // create & update system info
    let mut system = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    system.refresh_cpu_usage();
    system.refresh_memory();

    log::debug!("Getting system information...");
    let brand = system.cpus()[0].brand().to_string();
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
    log::info!("{} {} ({})", brand, os_name, os_version);

    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Model"),
        Cell::new("TPS"),
        Cell::new("CPU Usage (%)"),
        Cell::new("Total Memory (KB)"),
        Cell::new("Used Memory (KB)"),
    ]));

    // iterate over Ollama models
    for model in models
        .into_iter()
        .filter(|m| ModelProvider::from(m.clone()) == ModelProvider::Ollama)
    {
        log::debug!("Pulling model: {}", model);
        match ollama.pull_model(model.to_string(), false).await {
            Ok(status) => log::debug!("Status: {}", status.message),
            Err(err) => {
                log::error!("Failed to pull model {}: {:?}", model, err);
            }
        }

        match ollama
            .generate(GenerationRequest::new(
                model.to_string(),
                "Write a poem about Julius Caesar.".to_string(),
            ))
            .await
        {
            Ok(response) => {
                log::debug!("Got response for model {}", model);

                system.refresh_cpu_usage();
                system.refresh_memory();
                table.add_row(Row::new(vec![
                    Cell::new(&model.to_string()),
                    Cell::new(&get_response_tps(&response).to_string()),
                    Cell::new(&system.global_cpu_usage().to_string()),
                    Cell::new(&(system.total_memory() / 1000).to_string()),
                    Cell::new(&(system.used_memory() / 1000).to_string()),
                ]));
                // TODO: should add GPU usage here as well
            }
            Err(e) => {
                log::warn!("Ignoring model {}: Workflow failed with error {}", model, e);
            }
        }

        // wait a bit because CPU usage is based on diff
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
    }

    // print the final result
    table.printstd();
}

/// Computes the TPS.
#[inline(always)]
fn get_response_tps(res: &GenerationResponse) -> f64 {
    (res.eval_count.unwrap_or_default() as f64) / (res.eval_duration.unwrap_or(1) as f64)
        * 1_000_000_000f64
}

#[cfg(feature = "profiling")]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single() {
        env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("tps", log::LevelFilter::Debug)
            .filter_module("dkn_workflows", log::LevelFilter::Debug)
            .parse_default_env()
            .is_test(true)
            .init();

        let models = vec![Model::Llama3_2_3B, Model::Llama3_2_1B];

        let config = OllamaConfig::default();
        let ollama = Ollama::new(config.host, config.port);

        run_benchmark(ollama, models).await;
    }
}
