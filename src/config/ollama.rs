use std::time::Duration;

use ollama_workflows::{ollama_rs::Ollama, Executor, Model, ProgramMemory, Workflow};

const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
const DEFAULT_OLLAMA_PORT: u16 = 11434;

/// Some models such as small embedding models, are hardcoded into the node.
const HARDCODED_MODELS: [&str; 1] = ["hellord/mxbai-embed-large-v1:f16"];

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
        test_workflow_timeout: Duration,
    ) -> Result<Vec<Model>, String> {
        log::info!(
            "Checking Ollama requirements (auto-pull {}, workflow timeout: {}s)",
            if self.auto_pull { "on" } else { "off" },
            test_workflow_timeout.as_secs()
        );

        let ollama = Ollama::new(&self.host, self.port);

        // fetch local models
        let local_models = match ollama.list_local_models().await {
            Ok(models) => models.into_iter().map(|m| m.name).collect::<Vec<_>>(),
            Err(e) => {
                return {
                    log::error!("Could not fetch local models from Ollama, is it online?");
                    Err(e.to_string())
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
                .test_workflow(model.clone(), test_workflow_timeout)
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
    async fn try_pull(&self, ollama: &Ollama, model: String) -> Result<(), String> {
        log::warn!("Model {} not found in Ollama", model);
        if self.auto_pull {
            // if auto-pull is enabled, pull the model
            log::info!(
                "Downloading missing model {} (this may take a while)",
                model
            );
            let status = ollama
                .pull_model(model, false)
                .await
                .map_err(|e| format!("Error pulling model with Ollama: {}", e))?;
            log::debug!("Pulled model with Ollama, final status: {:#?}", status);
            Ok(())
        } else {
            // otherwise, give error
            log::error!("Please download missing model with: ollama pull {}", model);
            log::error!("Or, set OLLAMA_AUTO_PULL=true to pull automatically.");
            Err("Required model not pulled in Ollama.".into())
        }
    }

    /// Runs a small workflow to test Ollama Workflows.
    ///
    /// This is to see if a given system can execute Ollama workflows for their chosen models,
    /// e.g. if they have enough RAM/CPU and such.
    pub async fn test_workflow(&self, model: Model, timeout: Duration) -> bool {
        // this is the test workflow that we will run
        // TODO: when Workflow's have `Clone`, we can remove the repetitive parsing here
        let workflow = serde_json::from_value::<Workflow>(serde_json::json!({
            "name": "Simple",
            "description": "This is a simple workflow",
            "config":{
                "max_steps": 5,
                "max_time": 100,
                "max_tokens": 100,
                "tools": []
            },
            "tasks":[
                {
                    "id": "A",
                    "name": "Random Poem",
                    "description": "Writes a poem about Kapadokya.",
                    "prompt": "Please write a poem about Kapadokya.",
                    "inputs":[],
                    "operator": "generation",
                    "outputs":[
                        {
                            "type": "write",
                            "key": "poem",
                            "value": "__result"
                        }
                    ]
                },
                {
                    "id": "__end",
                    "name": "end",
                    "description": "End of the task",
                    "prompt": "End of the task",
                    "inputs": [],
                    "operator": "end",
                    "outputs": []
                }
            ],
            "steps":[
                {
                    "source":"A",
                    "target":"end"
                }
            ],
            "return_value":{
                "input":{
                    "type": "read",
                    "key": "poem"
                }
            }
        }))
        .expect("Preset workflow should be parsed");

        log::info!("Testing model {}", model);
        let executor = Executor::new_at(model.clone(), &self.host, self.port);
        let mut memory = ProgramMemory::new();
        tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                log::warn!("Ignoring model {}: Workflow timed out", model);
            },
            result = executor.execute(None, workflow, &mut memory) => {
                if result.is_empty() {
                    log::warn!("Ignoring model {}: Workflow returned empty result", model);
                } else {
                    log::info!("Accepting model {}", model);
                    return true;
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
        let model = Model::Phi3Mini.to_string();
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
        let exe = Executor::new(Model::Phi3Mini);
        let mut memory = ProgramMemory::new();

        let result = exe.execute(None, workflow, &mut memory).await;
        println!("Result: {}", result);
    }
}
