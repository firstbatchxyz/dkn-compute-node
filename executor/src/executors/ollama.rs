use dkn_utils::payloads::SpecModelPerformance;
use eyre::{Context, Result};
use ollama_rs::generation::completion::request::GenerationRequest;
use rig::completion::{Chat, PromptError};
use rig::providers::ollama;
use std::collections::HashMap;
use std::time::Duration;
use std::{collections::HashSet, env};

use crate::{Model, TaskBody};

const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
const DEFAULT_OLLAMA_PORT: u16 = 11434;

/// Timeout duration for checking model performance during a generation.
const PERFORMANCE_TIMEOUT: Duration = Duration::from_secs(120);
/// Minimum tokens per second (TPS) for checking model performance during a generation.
const PERFORMANCE_MIN_TPS: f64 = 15.0;

/// Ollama-specific configurations.
#[derive(Clone)]
pub struct OllamaClient {
    /// Whether to automatically pull models from Ollama.
    auto_pull: bool,
    /// Underlying Ollama client.
    client: ollama::Client,
    /// A more specialized Ollama client.
    ///
    /// - Can do pulls
    /// - Can list local models
    ollama_rs_client: ollama_rs::Ollama,
}

impl OllamaClient {
    /// Creates a new Ollama client using the host and port.
    pub fn new(host: &str, port: u16, auto_pull: bool) -> Self {
        Self {
            auto_pull,
            ollama_rs_client: ollama_rs::Ollama::new(host, port),
            client: ollama::Client::from_url(&format!("{host}:{port}",)),
        }
    }

    /// Looks at the environment variables for Ollama host and port.
    ///
    /// If not found, defaults to `DEFAULT_OLLAMA_HOST` and `DEFAULT_OLLAMA_PORT`.
    ///
    /// Returns a `Result` to be compatible with other executors.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let host = env::var("OLLAMA_HOST")
            .map(|h| h.trim_matches('"').to_string())
            .unwrap_or(DEFAULT_OLLAMA_HOST.to_string());
        let port = env::var("OLLAMA_PORT")
            .and_then(|port_str| port_str.parse().map_err(|_| std::env::VarError::NotPresent))
            .unwrap_or(DEFAULT_OLLAMA_PORT);

        // auto-pull, its true by default
        let auto_pull = env::var("OLLAMA_AUTO_PULL")
            .map(|s| s == "true")
            .unwrap_or(true);

        Ok(Self::new(&host, port, auto_pull))
    }

    /// Sets the auto-pull flag for Ollama models.
    pub fn with_auto_pull(mut self, auto_pull: bool) -> Self {
        self.auto_pull = auto_pull;
        self
    }

    pub async fn execute(&self, task: TaskBody) -> Result<String, PromptError> {
        let mut model = self.client.agent(&task.model.to_string());
        if let Some(preamble) = task.preamble {
            model = model.preamble(&preamble);
        }

        let agent = model.build();

        agent.chat(task.prompt, task.chat_history).await
    }

    /// Check if requested models exist in Ollama & test them using a dummy prompt.
    pub async fn check(
        &self,
        models: &mut HashSet<Model>,
    ) -> Result<HashMap<Model, SpecModelPerformance>> {
        log::info!(
            "Checking Ollama requirements (auto-pull {}, timeout: {}s, min tps: {})",
            if self.auto_pull { "on" } else { "off" },
            PERFORMANCE_TIMEOUT.as_secs(),
            PERFORMANCE_MIN_TPS
        );

        // fetch local models
        let local_models = match self.ollama_rs_client.list_local_models().await {
            Ok(models) => models.into_iter().map(|m| m.name).collect::<Vec<_>>(),
            Err(e) => {
                return {
                    log::error!("Could not fetch local models from Ollama, is it online?");
                    Err(e.into())
                }
            }
        };
        log::info!("Found local Ollama models: {:#?}", local_models);

        // check external models & pull them if available
        // iterate over models and remove bad ones
        let mut models_to_remove = Vec::new();
        let mut model_performances = HashMap::new();
        for model in models.iter() {
            // pull the model if it is not in the local models
            if !local_models.contains(&model.to_string()) {
                log::warn!("Model {} not found in Ollama", model);
                if self.auto_pull {
                    self.try_pull(model)
                        .await
                        .wrap_err("could not pull model")?;
                } else {
                    log::error!("Please download missing model with: ollama pull {}", model);
                    log::error!("Or, set OLLAMA_AUTO_PULL=true to pull automatically.");
                    eyre::bail!("required model not pulled in Ollama");
                }
            }

            // test its performance
            let perf = self.measure_tps_with_warmup(model).await;
            if let SpecModelPerformance::PassedWithTPS(_) = perf {
                model_performances.insert(*model, perf);
            } else {
                // if its anything but PassedWithTPS, remove the model
                models_to_remove.push(*model);
                model_performances.insert(*model, perf);
            }
        }

        // remove failed models
        for model in models_to_remove {
            models.remove(&model);
        }

        if models.is_empty() {
            log::warn!("No Ollama models passed the performance test! Try using a more powerful machine OR smaller models.");
        } else {
            log::info!("Ollama checks are finished, using models: {:#?}", models);
        }

        Ok(model_performances)
    }

    /// Pulls a model from Ollama.
    async fn try_pull(&self, model: &Model) -> Result<ollama_rs::models::pull::PullModelStatus> {
        // TODO: add pull-bar here
        // if auto-pull is enabled, pull the model
        log::info!(
            "Downloading missing model {} (this may take a while)",
            model
        );
        self.ollama_rs_client
            .pull_model(model.to_string(), false)
            .await
            .wrap_err("could not pull model")
    }

    /// Runs a small test to test local model performance.
    ///
    /// This is to see if a given system can execute tasks for their chosen models,
    /// e.g. if they have enough RAM/CPU and such.
    pub async fn measure_tps_with_warmup(&self, model: &Model) -> SpecModelPerformance {
        const TEST_PROMPT: &str = "Please write a poem about Kapadokya.";
        const WARMUP_PROMPT: &str = "Write a short poem about hedgehogs and squirrels.";

        log::info!("Testing model {}", model);

        // run a dummy generation for warm-up
        log::debug!("Warming up Ollama for model {}", model);
        if let Err(err) = self
            .ollama_rs_client
            .generate(GenerationRequest::new(
                model.to_string(),
                WARMUP_PROMPT.to_string(),
            ))
            .await
        {
            log::warn!("Ignoring model {model}: {err}");
            return SpecModelPerformance::ExecutionFailed;
        }

        // then, run a sample generation with timeout and measure tps
        let Ok(result) = tokio::time::timeout(
            PERFORMANCE_TIMEOUT,
            self.ollama_rs_client.generate(GenerationRequest::new(
                model.to_string(),
                TEST_PROMPT.to_string(),
            )),
        )
        .await
        else {
            log::warn!("Ignoring model {model}: Timed out");
            return SpecModelPerformance::Timeout;
        };

        // check the result
        match result {
            Ok(response) => {
                let tps = (response.eval_count.unwrap_or_default() as f64)
                    / (response.eval_duration.unwrap_or(1) as f64)
                    * 1_000_000_000f64;

                if tps >= PERFORMANCE_MIN_TPS {
                    log::info!("Model {model} passed the test with tps: {tps}");
                    SpecModelPerformance::PassedWithTPS(tps)
                } else {
                    log::warn!(
                        "Ignoring model {model}: tps too low ({tps:.3} < {:.3})",
                        PERFORMANCE_MIN_TPS
                    );
                    SpecModelPerformance::FailedWithTPS(tps)
                }
            }
            Err(err) => {
                log::warn!("Ignoring model {model} due to: {err}");
                SpecModelPerformance::ExecutionFailed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Ollama"]
    async fn test_ollama_prompt() {
        let client = OllamaClient::from_env().unwrap();
        let model = Model::Llama3_2_1bInstructQ4Km;

        let stats = client.try_pull(&model).await.unwrap();
        println!("Model {}: {:#?}", model, stats);
        let prompt = "The sky appears blue during the day because of a process called scattering. \
                    When sunlight enters the Earth's atmosphere, it collides with air molecules such as oxygen and nitrogen. \
                    These collisions cause some of the light to be absorbed or reflected, which makes the colors we see appear more vivid and vibrant. \
                    Blue is one of the brightest colors that is scattered the most by the atmosphere, making it visible to our eyes during the day. \
                    What may be the question this answer?".to_string();

        let response = client
            .execute(TaskBody::new_prompt(&prompt, model))
            .await
            .unwrap();

        println!("Prompt: {}\n\nResponse:{}", prompt, response);
    }
}
