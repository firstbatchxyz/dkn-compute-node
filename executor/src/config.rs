use std::collections::{HashMap, HashSet};

use crate::{executors::DriaExecutor, DriaExecutorError, Model, ModelProvider};
use eyre::Result;

#[derive(Clone)]
pub struct DriaExecutorsConfig {
    /// List of all models supported by this node.
    ///
    /// Equivalent to the union of all sets of models in the providers.
    pub models: HashSet<Model>,
    /// Providers and their executors along with the models they support.
    pub providers: HashMap<ModelProvider, (DriaExecutor, HashSet<Model>)>,
}

impl DriaExecutorsConfig {
    /// Creates a new config with the given models, using environment variables for the providers.
    pub fn new_from_env_for_models(models: Vec<Model>) -> Result<Self, std::env::VarError> {
        let mut provider_set = HashMap::new();
        let mut model_set = HashSet::new();
        for model in models {
            // get the provider for the model
            let provider = model.provider();

            // create a new executor for the provider if it does not exist
            //
            // we do this like this instead of `entry(key).or_insert` because we want to
            // return the error here
            if !provider_set.contains_key(&provider) {
                // create a new executor for the provider, may return an error!
                let executor = DriaExecutor::new_from_env(provider)?;
                provider_set.insert(provider, (executor, HashSet::new()));
            }

            // add the model to the provider models set
            provider_set.get_mut(&provider).map(|(_, models)| {
                models.insert(model);
            });

            // add the model to the global model set
            model_set.insert(model);
        }

        Ok(Self {
            providers: provider_set,
            models: model_set,
        })
    }

    /// Given the model, returns a _cloned_ executor for it.
    ///
    /// If the model's provider is not supported, an error is returned.
    /// Likewise, if the provider is supported but the model is not, an error is returned.
    pub async fn get_executor(&self, model: &Model) -> Result<DriaExecutor, DriaExecutorError> {
        let provider = model.provider();
        let (executor, models) = self
            .providers
            .get(&model.provider())
            .ok_or(DriaExecutorError::ProviderNotSupported(provider))?;

        if models.contains(model) {
            Ok(executor.clone())
        } else {
            Err(DriaExecutorError::ModelNotSupported(*model).into())
        }
    }

    /// Returns `true` if the configuration contains models that can be processed in parallel, e.g. API calls.
    ///
    /// This is not just a negation of `has_non_batchable_models`, as it also checks for the presence of models that can be run in parallel.
    pub fn has_batchable_models(&self) -> bool {
        self.providers.contains_key(&ModelProvider::Gemini)
            || self.providers.contains_key(&ModelProvider::OpenAI)
            || self.providers.contains_key(&ModelProvider::OpenRouter)
    }

    /// Returns `true` if the configuration contains a model that cant be run in parallel, e.g. a Ollama model.
    pub fn has_non_batchable_models(&self) -> bool {
        self.providers.contains_key(&ModelProvider::Ollama)
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

        // obtain the final list of providers & models, removing the providers with no models left
        self.providers.retain(|provider, (_, models)| {
            let ok = !models.is_empty();
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
