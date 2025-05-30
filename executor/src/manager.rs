use dkn_utils::payloads::SpecModelPerformance;

use crate::{executors::DriaExecutor, Model, ModelProvider};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct DriaExecutorsManager {
    /// List of all models supported by this node.
    ///
    /// Equivalent to the union of all sets of models in the providers.
    pub models: HashSet<Model>,
    /// Providers and their executors along with the models they support.
    pub providers: HashMap<ModelProvider, (DriaExecutor, HashSet<Model>)>,
}

impl DriaExecutorsManager {
    /// Creates a new executor manager with the given models, using environment variables for the providers.
    ///
    /// If a provider is required (as per the chosen model) but its environment variables are missing,
    /// this will return an error.
    pub fn new_from_env_for_models(
        models: impl Iterator<Item = Model>,
    ) -> Result<Self, std::env::VarError> {
        let mut provider_set: HashMap<ModelProvider, (DriaExecutor, HashSet<Model>)> =
            HashMap::new();
        let mut model_set = HashSet::new();
        for model in models {
            // get the provider for the model
            let provider = model.provider();

            // add model to the provider set, and create a new executor if needed
            match provider_set.get_mut(&provider) {
                Some((_, models)) => {
                    models.insert(model);
                }
                None => {
                    // create a new executor for the provider, may return an error!
                    let executor = DriaExecutor::new_from_env(provider)?;
                    provider_set.insert(provider, (executor, HashSet::from_iter([model])));
                }
            }

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
    pub async fn get_executor(&self, model: &Model) -> eyre::Result<DriaExecutor> {
        let provider = model.provider();
        let (executor, models) = self
            .providers
            .get(&provider)
            .ok_or_else(|| eyre::eyre!("Provider {provider} supported by this executor"))?;

        if models.contains(model) {
            Ok(executor.clone())
        } else {
            Err(eyre::eyre!("Model {model} not supported by this executor"))
        }
    }

    /// Returns the set of models supported by the given provider for this manager.
    ///
    /// If there are no models for the provider, an empty set is returned.
    pub fn get_models_for_provider(&self, provider: ModelProvider) -> HashSet<Model> {
        self.providers
            .get(&provider)
            .map(|(_, models)| models.clone())
            .unwrap_or_default()
    }

    /// Returns the names of all models in the manager, in a random order.
    pub fn get_model_names(&self) -> Vec<String> {
        self.models.iter().map(|m| m.to_string()).collect()
    }

    /// Check if the required compute services are running.
    ///
    /// - If Ollama models are used the task is tested with a simple task with timeout.
    /// - If API based models are used, the API key is checked and the models are tested with a dummy request.
    ///
    /// In the end, bad models are filtered out and we simply check if we are left if any valid models at all.
    /// If there are no models left in the end, an error is thrown.
    pub async fn check_services(&mut self) -> eyre::Result<HashMap<Model, SpecModelPerformance>> {
        log::info!("Checking configured services.");

        // check all configured providers & record model performances
        let mut model_perf = HashMap::new();
        for (client, models) in self.providers.values_mut() {
            let provider_model_perf = client.check(models).await?;
            model_perf.extend(provider_model_perf);
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

        Ok(model_perf)
    }
}
