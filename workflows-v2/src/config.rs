use std::collections::{HashMap, HashSet};

use crate::{providers::DriaWorkflowsProvider, Model, ModelProvider, TaskBody};
use dkn_utils::split_csv_line;
use eyre::{eyre, OptionExt, Result};
use rand::seq::IteratorRandom;

#[derive(Clone)]
pub struct DriaWorkflowsConfig {
    pub models: Vec<Model>,
    /// Providers and their clients, as derived from the models.
    pub providers: HashMap<ModelProvider, (DriaWorkflowsProvider, HashSet<Model>)>,
}

impl DriaWorkflowsConfig {
    /// Creates a new config with the given models.
    pub fn new(models: Vec<Model>) -> Self {
        // given a vector of models, creates a map of providers and its models
        let providers = models.iter().cloned().fold(
            HashMap::<ModelProvider, (DriaWorkflowsProvider, HashSet<Model>)>::new(),
            |mut provider_models, model| {
                let provider = model.provider();
                provider_models
                    .entry(model.provider())
                    // create a new provider if it does not exist
                    .or_insert_with(|| {
                        (
                            DriaWorkflowsProvider::new(provider),
                            HashSet::from_iter([model]),
                        )
                    })
                    // or append the model set to the existing provider
                    .1
                    .insert(model);
                provider_models
            },
        );

        Self { providers, models }
    }

    /// Executes a given task using the appropriate provider.
    pub async fn execute(&self, task: TaskBody) -> Result<String, rig::completion::PromptError> {
        self.providers
            .get(&task.model.provider())
            .unwrap() // TODO: give error here
            .0
            .execute(task)
            .await
    }

    /// Parses Ollama-Workflows compatible models from a comma-separated values string.
    pub fn new_from_csv(input: &str) -> Self {
        let models_str = split_csv_line(input);

        let models = models_str
            .into_iter()
            .filter_map(|s| Model::try_from(s).ok())
            .collect();

        Self::new(models)
    }

    /// Returns the models from the config that belongs to a given provider.
    pub fn get_models_for_provider(&self, provider: ModelProvider) -> &HashSet<Model> {
        &self
            .providers
            .get(&provider)
            .unwrap() // TODO: give error here
            .1
    }

    /// Returns `true` if the configuration contains models that can be processed in parallel, e.g. API calls.
    pub fn has_batchable_models(&self) -> bool {
        !self.has_non_batchable_models()
    }

    /// Returns `true` if the configuration contains a model that cant be run in parallel, e.g. a Ollama model.
    pub fn has_non_batchable_models(&self) -> bool {
        self.providers.contains_key(&ModelProvider::Ollama)
    }

    /// Given a raw model name or provider (as a string), returns the first matching model & provider.
    ///
    /// - If input is `*` or `all`, a random model is returned.
    /// - If input is a model and is supported by this node, it is returned directly.
    /// - If input is a provider, the first matching model in the node config is returned.
    ///
    /// If there are no matching models with this logic, an error is returned.
    pub fn get_matching_model(&self, model_or_provider: String) -> Result<Model> {
        if model_or_provider == "*" {
            // return a random model
            self.models
                .iter()
                .next() // HashSet iterates randomly, so we just pick the first
                .ok_or_eyre("could not find models to randomly pick for '*'")
                .cloned()
        } else if let Ok(provider) = ModelProvider::try_from(model_or_provider.clone()) {
            // this is a valid provider, return the first matching model in the config
            self.models
                .iter()
                .find(|&m| m.provider() == provider)
                .ok_or_eyre(format!(
                    "Provider {provider} is not supported by this node."
                ))
                .cloned()
        } else if let Ok(model) = Model::try_from(model_or_provider.clone()) {
            // this is a valid model, return it if it is supported by the node
            self.models
                .iter()
                .find(|&m| *m == model)
                .ok_or_eyre(format!("Model {model} is not supported by this node."))
                .cloned()
        } else {
            // this is neither a valid provider or model for this node
            Err(eyre!(
                "Given string '{model_or_provider}' is neither a model nor provider.",
            ))
        }
    }

    /// From a list of model or provider names, return a random matching model & provider.
    ///
    /// FIXME: refactor this
    pub fn get_any_matching_model(&self, list_model_or_provider: Vec<String>) -> Result<Model> {
        // filter models w.r.t supported ones
        let matching_models = list_model_or_provider
            .into_iter()
            .filter_map(|model_or_provider| {
                let result = self.get_matching_model(model_or_provider);
                match result {
                    Ok(result) => Some(result),
                    Err(e) => {
                        log::debug!("Ignoring model: {}", e);
                        None
                    }
                }
            })
            .collect::<Vec<_>>();

        // choose random model
        matching_models
            .into_iter()
            .choose(&mut rand::thread_rng())
            .ok_or(eyre!("No matching models found."))
    }

    /// Returns the list of unique providers in the config.
    #[inline]
    pub fn get_providers(&self) -> Vec<ModelProvider> {
        self.providers.keys().cloned().collect()
    }

    /// Returns the names of all models in the config.
    #[inline(always)]
    pub fn get_model_names(&self) -> Vec<String> {
        self.models.iter().map(|m| m.to_string()).collect()
    }

    /// Check if the required compute services are running.
    ///
    /// - If Ollama models are used, hardcoded models are checked locally, and for
    ///   external models, the workflow is tested with a simple task with timeout.
    /// - If API based models are used, the API key is checked and the models are tested with a dummy request.
    ///
    /// In the end, bad models are filtered out and we simply check if we are left if any valid models at all.
    /// If there are no models left in the end, an error is thrown.
    pub async fn check_services(&mut self) -> Result<()> {
        log::info!("Checking configured services.");

        // check all configured providers
        for (client, models) in self.providers.values_mut() {
            client.check(models).await?;
        }

        // obtain the final list of providers & models,
        // remove the providers that have no models left
        self.providers.retain(|provider, (_, models)| {
            let ok = models.is_empty();
            if !ok {
                log::warn!(
                    "Provider {} has no models left, removing it from the config.",
                    provider
                )
            }
            ok
        });

        // check if we have any models left at all
        if self.providers.is_empty() {
            eyre::bail!("No good models found, please check logs for errors.")
        }

        Ok(())
    }
}

impl std::fmt::Display for DriaWorkflowsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let models_str = self
            .models
            .iter()
            .map(|model| format!("{}:{}", model.provider(), model))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}", models_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "TODO: fix this test"]
    fn test_csv_parser() {
        let cfg = DriaWorkflowsConfig::new_from_csv("idontexist,i dont either,i332287648762");
        assert_eq!(cfg.models.len(), 0);

        let cfg = DriaWorkflowsConfig::new_from_csv("gemma2:9b-instruct-q8_0,gpt-4o,balblablabl");
        assert_eq!(cfg.models.len(), 1);
    }

    #[test]
    #[ignore = "TODO: fix this test"]
    fn test_model_matching() {
        let cfg = DriaWorkflowsConfig::new_from_csv("gpt-4o,llama3.2:1b-instruct-q4_K_M");
        assert_eq!(
            cfg.get_matching_model("openai".to_string()).unwrap(),
            Model::GPT4o,
            "Should find existing model"
        );

        assert_eq!(
            cfg.get_matching_model("llama3.2:1b-instruct-q4_K_M".to_string())
                .unwrap(),
            Model::Llama3_2_1bInstructQ4Km,
            "Should find existing model"
        );

        assert!(
            cfg.get_matching_model("gpt-4o-mini".to_string()).is_err(),
            "Should not find anything for unsupported model"
        );

        assert!(
            cfg.get_matching_model("praise the model".to_string())
                .is_err(),
            "Should not find anything for inexisting model"
        );
    }

    #[test]
    #[ignore = "TODO: fix this test"]
    fn test_get_any_matching_model() {
        let cfg = DriaWorkflowsConfig::new_from_csv("gpt-4o-mini,llama3.2:1b-instruct-q4_K_M");
        let result = cfg.get_any_matching_model(vec![
            "i-dont-exist".to_string(),
            "llama3.1:latest".to_string(),
            "gpt-4o".to_string(),
            "ollama".to_string(),
        ]);
        assert_eq!(
            result.unwrap(),
            Model::Llama3_2_1bInstructQ4Km,
            "Should find existing model"
        );
    }
}
