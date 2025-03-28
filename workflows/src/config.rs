use crate::{
    apis::{JinaConfig, SerperConfig},
    providers::{GeminiConfig, OllamaConfig, OpenAIConfig, OpenRouterConfig},
    Model, ModelProvider,
};
use dkn_utils::split_csv_line;
use eyre::{eyre, OptionExt, Result};
use rand::seq::IteratorRandom; // provides Vec<_>.choose

#[derive(Debug, Clone)]
pub struct DriaWorkflowsConfig {
    /// List of models.
    ///
    /// You can do `model.provider()` to get its provider.
    pub models: Vec<Model>,
    /// Ollama configurations, in case Ollama is used.
    /// Otherwise, can be ignored.
    pub ollama: OllamaConfig,
    /// OpenAI configurations, e.g. API key, in case OpenAI is used.
    /// Otherwise, can be ignored.
    pub openai: OpenAIConfig,
    /// Gemini configurations, e.g. API key, in case Gemini is used.
    /// Otherwise, can be ignored.
    pub gemini: GeminiConfig,
    /// OpenRouter configurations, e.g. API key, in case OpenRouter is used.
    /// Otherwise, can be ignored.
    pub openrouter: OpenRouterConfig,
    /// Serper configurations, e.g. API key, in case Serper is given in environment.
    /// Otherwise, can be ignored.
    pub serper: SerperConfig,
    /// Jina configurations, e.g. API key, in case Jina is used.
    /// Otherwise, can be ignored.
    pub jina: JinaConfig,
}

impl Default for DriaWorkflowsConfig {
    fn default() -> Self {
        Self::new(Vec::default())
    }
}

impl DriaWorkflowsConfig {
    /// Creates a new config with the given models.
    pub fn new(models: Vec<Model>) -> Self {
        Self {
            models,
            ollama: OllamaConfig::new(),
            openai: OpenAIConfig::new(),
            openrouter: OpenRouterConfig::new(),
            gemini: GeminiConfig::new(),
            serper: SerperConfig::new(),
            jina: JinaConfig::new(),
        }
    }

    /// Sets the Ollama configuration for the Workflows config.
    pub fn with_ollama_config(mut self, ollama: OllamaConfig) -> Self {
        self.ollama = ollama;
        self
    }

    /// Sets the OpenAI configuration for the Workflows config.
    pub fn with_openai_config(mut self, openai: OpenAIConfig) -> Self {
        self.openai = openai;
        self
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
    pub fn get_models_for_provider(&self, provider: ModelProvider) -> Vec<Model> {
        self.models
            .iter()
            .filter(|m| m.provider() == provider)
            .cloned()
            .collect()
    }

    /// Returns `true` if the configuration contains models that can be processed in parallel, e.g. API calls.
    pub fn has_batchable_models(&self) -> bool {
        self.models
            .iter()
            .any(|m| m.provider() != ModelProvider::Ollama)
    }

    /// Returns `true` if the configuration contains a model that cant be run in parallel, e.g. a Ollama model.
    pub fn has_non_batchable_models(&self) -> bool {
        self.models
            .iter()
            .any(|m| m.provider() == ModelProvider::Ollama)
    }

    /// Given a raw model name or provider (as a string), returns the first matching model & provider.
    ///
    /// - If input is `*` or `all`, a random model is returned.
    /// - if input is `!` the first model is returned.
    /// - If input is a model and is supported by this node, it is returned directly.
    /// - If input is a provider, the first matching model in the node config is returned.
    ///
    /// If there are no matching models with this logic, an error is returned.
    pub fn get_matching_model(&self, model_or_provider: String) -> Result<Model> {
        if model_or_provider == "*" {
            // return a random model
            self.models
                .iter()
                .choose(&mut rand::thread_rng())
                .ok_or_eyre("could not find models to randomly pick for '*'")
                .cloned()
        } else if model_or_provider == "!" {
            // return the first model
            self.models
                .first()
                .ok_or_eyre("could not find models to choose first for '!'")
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
        self.models.iter().fold(Vec::new(), |mut unique, m| {
            let provider = m.provider();

            if !unique.contains(&provider) {
                unique.push(provider);
            }

            unique
        })
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

        // check Serper
        self.serper.check_optional().await?;

        // check Jina
        self.jina.check_optional().await?;

        // TODO: can refactor (provider, model) logic here
        let unique_providers = self.get_providers();

        let mut good_models = Vec::new();

        // if Ollama is a provider, check that it is running & Ollama models are pulled (or pull them)
        if unique_providers.contains(&ModelProvider::Ollama) {
            let provider_models = self.get_models_for_provider(ModelProvider::Ollama);
            good_models.extend(self.ollama.check(provider_models).await?);
        }

        // if OpenAI is a provider, check that the API key is set & models are available
        if unique_providers.contains(&ModelProvider::OpenAI) {
            let provider_models = self.get_models_for_provider(ModelProvider::OpenAI);
            good_models.extend(self.openai.check(provider_models).await?);
        }

        // if Gemini is a provider, check that the API key is set & models are available
        if unique_providers.contains(&ModelProvider::Gemini) {
            let provider_models = self.get_models_for_provider(ModelProvider::Gemini);
            good_models.extend(self.gemini.check(provider_models).await?);
        }

        // if OpenRouter is a provider, check that the API key is set
        if unique_providers.contains(&ModelProvider::OpenRouter) {
            let provider_models = self.get_models_for_provider(ModelProvider::OpenRouter);
            good_models.extend(self.openrouter.check(provider_models).await?);
        }

        // update good models
        if good_models.is_empty() {
            Err(eyre!("No good models found, please check logs for errors."))
        } else {
            self.models = good_models;
            Ok(())
        }
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
    fn test_csv_parser() {
        let cfg = DriaWorkflowsConfig::new_from_csv("idontexist,i dont either,i332287648762");
        assert_eq!(cfg.models.len(), 0);

        let cfg = DriaWorkflowsConfig::new_from_csv(
            "gemma2:9b-instruct-q8_0,phi3:14b-medium-4k-instruct-q4_1,balblablabl",
        );
        assert_eq!(cfg.models.len(), 2);
    }

    #[test]
    fn test_model_matching() {
        let cfg = DriaWorkflowsConfig::new_from_csv("gpt-4o,llama3.1:latest");
        assert_eq!(
            cfg.get_matching_model("openai".to_string()).unwrap(),
            Model::GPT4o,
            "Should find existing model"
        );

        assert_eq!(
            cfg.get_matching_model("llama3.1:latest".to_string())
                .unwrap(),
            Model::Llama3_1_8B,
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
    fn test_get_any_matching_model() {
        let cfg = DriaWorkflowsConfig::new_from_csv("gpt-3.5-turbo,llama3.1:latest");
        let result = cfg.get_any_matching_model(vec![
            "i-dont-exist".to_string(),
            "llama3.1:latest".to_string(),
            "gpt-4o".to_string(),
            "ollama".to_string(),
        ]);
        assert_eq!(
            result.unwrap(),
            Model::Llama3_1_8B,
            "Should find existing model"
        );
    }
}
