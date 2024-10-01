use std::time::Duration;

use eyre::{eyre, Result};
use ollama_workflows::{
    ollama_rs::{
        generation::{
            completion::request::GenerationRequest,
            embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
            options::GenerationOptions,
        },
        Ollama,
    },
    Model,
};

const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
const DEFAULT_OLLAMA_PORT: u16 = 11434;

/// Some models such as small embedding models, are hardcoded into the node.
const HARDCODED_MODELS: [&str; 1] = ["hellord/mxbai-embed-large-v1:f16"];

/// Prompt to be used to see Ollama performance.
const TEST_PROMPT: &str = "Please write a poem about Kapadokya.";

/// Ollama-specific configurations.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Host, usually `http://127.0.0.1`.
    pub(crate) host: String,
    /// Port, usually `11434`.
    pub(crate) port: u16,
    /// List of hardcoded models that are internally used by Ollama workflows.
    hardcoded_models: Vec<String>,
    /// Whether to automatically pull models from Ollama.
    /// This is useful for CI/CD workflows.
    auto_pull: bool,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_OLLAMA_HOST.to_string(),
            port: DEFAULT_OLLAMA_PORT,
            hardcoded_models: HARDCODED_MODELS
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            auto_pull: false,
        }
    }
}
impl OllamaConfig {
    /// Looks at the environment variables for Ollama host and port.
    ///
    /// If not found, defaults to `DEFAULT_OLLAMA_HOST` and `DEFAULT_OLLAMA_PORT`.
    pub fn new() -> Self {
        let host = std::env::var("OLLAMA_HOST")
            .map(|h| h.trim_matches('"').to_string())
            .unwrap_or(DEFAULT_OLLAMA_HOST.to_string());
        let port = std::env::var("OLLAMA_PORT")
            .and_then(|port_str| port_str.parse().map_err(|_| std::env::VarError::NotPresent))
            .unwrap_or(DEFAULT_OLLAMA_PORT);

        // Ollama workflows may require specific models to be loaded regardless of the choices
        let hardcoded_models = HARDCODED_MODELS.iter().map(|s| s.to_string()).collect();

        let auto_pull = std::env::var("OLLAMA_AUTO_PULL")
            .map(|s| s == "true")
            .unwrap_or_default();

        Self {
            host,
            port,
            hardcoded_models,
            auto_pull,
        }
    }

    /// Check if requested models exist in Ollama, and then tests them using a workflow.
    pub async fn check(
        &self,
        external_models: Vec<Model>,
        timeout: Duration,
        min_tps: f64,
    ) -> Result<Vec<Model>> {
        log::info!(
            "Checking Ollama requirements (auto-pull {}, workflow timeout: {}s)",
            if self.auto_pull { "on" } else { "off" },
            timeout.as_secs()
        );

        let ollama = Ollama::new(&self.host, self.port);

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
        log::debug!("Checking hardcoded models: {:#?}", self.hardcoded_models);
        // only check if model is contained in local_models
        // we dont check workflows for hardcoded models
        for model in &self.hardcoded_models {
            if !local_models.contains(model) {
                self.try_pull(&ollama, model.to_owned()).await?;
            }
        }

        // check external models & pull them if available
        // and also run a test workflow for them
        let mut good_models = Vec::new();
        for model in external_models {
            if !local_models.contains(&model.to_string()) {
                self.try_pull(&ollama, model.to_string()).await?;
            }

            if self
                .test_performance(&ollama, &model, timeout, min_tps)
                .await
            {
                good_models.push(model);
            }
        }

        log::info!(
            "Ollama checks are finished, using models: {:#?}",
            good_models
        );
        Ok(good_models)
    }

    /// Pulls a model if `auto_pull` exists, otherwise returns an error.
    async fn try_pull(&self, ollama: &Ollama, model: String) -> Result<()> {
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
            Err(eyre!("Required model not pulled in Ollama."))
        }
    }

    /// Runs a small workflow to test Ollama Workflows.
    ///
    /// This is to see if a given system can execute Ollama workflows for their chosen models,
    /// e.g. if they have enough RAM/CPU and such.
    pub async fn test_performance(
        &self,
        ollama: &Ollama,
        model: &Model,
        timeout: Duration,
        min_tps: f64,
    ) -> bool {
        log::info!("Testing model {}", model);

        // first generate a dummy embedding to load the model into memory (warm-up)
        let request = GenerateEmbeddingsRequest::new(
            model.to_string(),
            EmbeddingsInput::Single("embedme".into()),
        );
        if let Err(err) = ollama.generate_embeddings(request).await {
            log::error!("Failed to generate embedding for model {}: {}", model, err);
            return false;
        };

        let mut generation_request =
            GenerationRequest::new(model.to_string(), TEST_PROMPT.to_string());

        // FIXME: temporary workaround, can take num threads from outside
        if let Ok(num_thread) = std::env::var("OLLAMA_NUM_THREAD") {
            generation_request = generation_request.options(
                GenerationOptions::default().num_thread(
                    num_thread
                        .parse()
                        .expect("num threads should be a positive integer"),
                ),
            );
        }

        // then, run a sample generation with timeout and measure tps
        tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                log::warn!("Ignoring model {}: Workflow timed out", model);
            },
            result = ollama.generate(generation_request) => {
                match result {
                    Ok(response) => {
                        let tps = (response.eval_count.unwrap_or_default() as f64)
                        / (response.eval_duration.unwrap_or(1) as f64)
                        * 1_000_000_000f64;

                        if tps >= min_tps {
                            log::info!("Model {} passed the test with tps: {}", model, tps);
                            return true;
                        }

                        log::warn!(
                            "Ignoring model {}: tps too low ({:.3} < {:.3})",
                            model,
                            tps,
                            min_tps
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
    #[ignore = "run this manually"]
    async fn test_ollama_prompt() {
        let model = Model::default().to_string();
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
    #[ignore = "run this manually"]
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
        let exe = Executor::new(Model::default());
        let mut memory = ProgramMemory::new();

        let result = exe.execute(None, workflow, &mut memory).await;
        println!("Result: {}", result.unwrap());
    }
}
