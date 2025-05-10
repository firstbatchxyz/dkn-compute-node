use eyre::{eyre, Context, Result};
use ollama_workflows::{
    ollama_rs::{generation::completion::request::GenerationRequest, Ollama},
    Model,
};
use std::env;
use std::time::Duration;

const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
const DEFAULT_OLLAMA_PORT: u16 = 11434;
/// Automatically pull missing models by default?
const DEFAULT_AUTO_PULL: bool = true;
/// Timeout duration for checking model performance during a generation.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(80);
/// Minimum tokens per second (TPS) for checking model performance during a generation.
const DEFAULT_MIN_TPS: f64 = 15.0;

/// Some models such as small embedding models, are hardcoded into the node.
const HARDCODED_MODELS: [&str; 1] = ["hellord/mxbai-embed-large-v1:f16"];
/// Prompt to be used to see Ollama performance.
const TEST_PROMPT: &str = "Please write a poem about Kapadokya.";

/// Ollama-specific configurations.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Host, usually `http://127.0.0.1`.
    pub host: String,
    /// Port, usually `11434`.
    pub port: u16,
    /// Whether to automatically pull models from Ollama.
    /// This is useful for CI/CD workflows.
    auto_pull: bool,
    /// Timeout duration for checking model performance during a generation.
    timeout: Duration,
    /// Minimum tokens per second (TPS) for checking model performance during a generation.
    min_tps: f64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_OLLAMA_HOST.to_string(),
            port: DEFAULT_OLLAMA_PORT,
            auto_pull: DEFAULT_AUTO_PULL,
            timeout: DEFAULT_TIMEOUT,
            min_tps: DEFAULT_MIN_TPS,
        }
    }
}
impl OllamaConfig {
    /// Looks at the environment variables for Ollama host and port.
    ///
    /// If not found, defaults to `DEFAULT_OLLAMA_HOST` and `DEFAULT_OLLAMA_PORT`.
    pub fn new() -> Self {
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

        Self {
            host,
            port,
            auto_pull,
            ..Default::default()
        }
    }

    /// Sets the timeout duration for checking model performance during a generation.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the minimum tokens per second (TPS) for checking model performance during a generation.
    pub fn with_min_tps(mut self, min_tps: f64) -> Self {
        self.min_tps = min_tps;
        self
    }

    /// Sets the auto-pull flag for Ollama models.
    pub fn with_auto_pull(mut self, auto_pull: bool) -> Self {
        self.auto_pull = auto_pull;
        self
    }

    /// Check if requested models exist in Ollama, and then tests them using a workflow.
    pub async fn check(&self, external_models: Vec<Model>) -> Result<Vec<Model>> {
        log::info!(
            "Checking Ollama requirements (auto-pull {}, timeout: {}s, min tps: {})",
            if self.auto_pull { "on" } else { "off" },
            self.timeout.as_secs(),
            self.min_tps
        );

        let ollama = Ollama::new(&self.host, self.port);
        log::info!("Connecting to Ollama at {}", ollama.url_str());

        // fetch local models
        let local_models = match ollama.list_local_models().await {
            Ok(models) => models.into_iter().map(|m| m.name).collect::<Vec<_>>(),
            Err(e) => {
                return {
                    log::error!("Could not fetch local models from Ollama, is it online?");
                    Err(e.into())
                }
            }
        };
        log::info!("Found local Ollama models: {:#?}", local_models);

        // check hardcoded models & pull them if available
        // these are not used directly by the user, but are needed for the workflows
        // we only check if model is contained in local_models, we dont check workflows for these
        for model in HARDCODED_MODELS {
            // `contains` doesnt work for &str so we equality check instead
            if !&local_models.iter().any(|s| s == model) {
                self.try_pull(&ollama, model.to_owned())
                    .await
                    .wrap_err("could not pull model")?;
            }
        }

        // check external models & pull them if available
        // and also run a test workflow for them
        let mut good_models = Vec::new();
        for model in external_models {
            if !local_models.contains(&model.to_string()) {
                self.try_pull(&ollama, model.to_string())
                    .await
                    .wrap_err("could not pull model")?;
            }

            if self.test_performance(&ollama, &model).await {
                good_models.push(model);
            }
        }

        if good_models.is_empty() {
            log::warn!("No Ollama models passed the performance test! Try using a more powerful machine OR smaller models.");
        } else {
            log::info!(
                "Ollama checks are finished, using models: {:#?}",
                good_models
            );
        }

        Ok(good_models)
    }

    /// Pulls a model if `auto_pull` exists, otherwise returns an error.
    async fn try_pull(&self, ollama: &Ollama, model: String) -> Result<()> {
        // TODO: add pull-bar here
        log::warn!("Model {} not found in Ollama", model);
        if self.auto_pull {
            // if auto-pull is enabled, pull the model
            log::info!(
                "Downloading missing model {} (this may take a while)",
                model
            );
            let status = ollama.pull_model(model, false).await?;
            log::debug!("Pulled model with Ollama, final status: {:#?}", status);
            Ok(())
        } else {
            // otherwise, give error
            log::error!("Please download missing model with: ollama pull {}", model);
            log::error!("Or, set OLLAMA_AUTO_PULL=true to pull automatically.");
            Err(eyre!("required model not pulled in Ollama"))
        }
    }

    /// Runs a small workflow to test Ollama Workflows.
    ///
    /// This is to see if a given system can execute Ollama workflows for their chosen models,
    /// e.g. if they have enough RAM/CPU and such.
    pub async fn test_performance(&self, ollama: &Ollama, model: &Model) -> bool {
        log::info!("Testing model {}", model);

        let generation_request = GenerationRequest::new(model.to_string(), TEST_PROMPT.to_string());

        // run a dummy generation for warm-up
        log::debug!("Warming up Ollama for model {}", model);
        if let Err(e) = ollama.generate(generation_request.clone()).await {
            log::warn!("Ignoring model {}: Workflow failed with error {}", model, e);
            return false;
        }

        // then, run a sample generation with timeout and measure tps
        tokio::select! {
            _ = tokio::time::sleep(self.timeout) => {
                log::warn!("Ignoring model {}: Workflow timed out", model);
            },
            result = ollama.generate(generation_request) => {
                match result {
                    Ok(response) => {
                        let tps = (response.eval_count.unwrap_or_default() as f64)
                        / (response.eval_duration.unwrap_or(1) as f64)
                        * 1_000_000_000f64;

                        if tps >= self.min_tps {
                            log::info!("Model {} passed the test with tps: {}", model, tps);
                            return true;
                        }

                        log::warn!(
                            "Ignoring model {}: tps too low ({:.3} < {:.3})",
                            model,
                            tps,
                            self.min_tps
                        );
                    }
                    Err(e) => {
                        log::warn!("Ignoring model {}: Workflow failed with error {}", model, e);
                    }
                }
            }
        };

        false
    }
}

#[cfg(test)]
mod tests {
    use ollama_workflows::ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
    use ollama_workflows::{Executor, Model, ProgramMemory, Workflow};

    #[tokio::test]
    #[ignore = "requires Ollama"]
    async fn test_ollama_prompt() {
        let model = Model::Llama3_3_70bInstructQ4Km.to_string();
        let ollama = Ollama::default();
        ollama.pull_model(model.clone(), false).await.unwrap();
        let prompt = "The sky appears blue during the day because of a process called scattering. \
                    When sunlight enters the Earth's atmosphere, it collides with air molecules such as oxygen and nitrogen. \
                    These collisions cause some of the light to be absorbed or reflected, which makes the colors we see appear more vivid and vibrant. \
                    Blue is one of the brightest colors that is scattered the most by the atmosphere, making it visible to our eyes during the day. \
                    What may be the question this answer?".to_string();

        let response = ollama
            .generate(GenerationRequest::new(model, prompt.clone()))
            .await
            .expect("Should generate response");
        println!("Prompt: {}\n\nResponse:{}", prompt, response.response);
    }

    #[tokio::test]
    #[ignore = "requires Ollama"]
    async fn test_ollama_workflow() {
        let workflow = r#"{
        "name": "Simple",
        "description": "This is a simple workflow",
        "config": {
            "max_steps": 5,
            "max_time": 100,
        },
        "tasks":[
            {
                "id": "A",
                "name": "Random Poem",
                "description": "Writes a poem about Kapadokya.",
                "prompt": "Please write a poem about Kapadokya.",
                "operator": "generation",
                "outputs": [
                    {
                        "type": "write",
                        "key": "final_result",
                        "value": "__result"
                    }
                ]
            },
            {
                "id": "__end",
                "name": "end",
                "description": "End of the task",
                "prompt": "End of the task",
                "operator": "end",
            }
        ],
        "steps":[
            {
                "source":"A",
                "target":"end"
            }
        ]
    }"#;
        let workflow: Workflow = serde_json::from_str(workflow).unwrap();
        let exe = Executor::new(Model::Llama3_3_70bInstructQ4Km);
        let mut memory = ProgramMemory::new();

        let result = exe.execute(None, &workflow, &mut memory).await;
        println!("Result: {}", result.unwrap());
    }
}
