use crate::{utils::split_comma_separated, OllamaConfig, OpenAIConfig};
use eyre::{eyre, Result};
use ollama_workflows::{Model, ModelProvider};
use rand::seq::IteratorRandom; // provides Vec<_>.choose

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub models: Vec<(ModelProvider, Model)>,
    pub ollama: OllamaConfig,
    pub openai: OpenAIConfig,
}

impl std::fmt::Display for ModelConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let models_str = self
            .models
            .iter()
            .map(|(provider, model)| format!("{:?}:{}", provider, model))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}", models_str)
    }
}

impl ModelConfig {
    /// Creates a new config with the given list of models.
    pub fn new(models: Vec<Model>) -> Self {
        // map models to (provider, model) pairs
        let models_providers = models
            .into_iter()
            .map(|m| (m.clone().into(), m))
            .collect::<Vec<_>>();

        let mut providers = Vec::new();

        // get ollama models & config
        let ollama_models = models_providers
            .iter()
            .filter_map(|(p, m)| {
                if *p == ModelProvider::Ollama {
                    Some(m.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let ollama_config = if !ollama_models.is_empty() {
            providers.push(ModelProvider::Ollama);
            Some(OllamaConfig::new(ollama_models))
        } else {
            None
        };

        // get openai models & config
        let openai_models = models_providers
            .iter()
            .filter_map(|(p, m)| {
                if *p == ModelProvider::OpenAI {
                    Some(m.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let openai_config = if !openai_models.is_empty() {
            providers.push(ModelProvider::OpenAI);
            Some(OpenAIConfig::new(openai_models))
        } else {
            None
        };

        Self {
            models_providers,
            providers,
            ollama_config,
            openai_config,
        }
    }

    /// Parses Ollama-Workflows compatible models from a comma-separated values string.
    ///
    /// ## Example
    ///
    /// ```
    /// let config = ModelConfig::new_from_csv("gpt-4-turbo,gpt-4o-mini");
    /// ```
    pub fn new_from_csv(input: Option<String>) -> Self {
        let models_str = split_comma_separated(input);

        let models = models_str
            .into_iter()
            .filter_map(|s| match Model::try_from(s) {
                Ok(model) => Some((model.clone().into(), model)),
                Err(e) => {
                    log::warn!("Error parsing model: {}", e);
                    None
                }
            })
            .collect::<Vec<_>>();

        Self { models }
    }

    /// Returns the models that belong to a given providers from the config.
    pub fn get_models_for_provider(&self, provider: ModelProvider) -> Vec<Model> {
        self.models
            .iter()
            .filter_map(|(p, m)| {
                if *p == provider {
                    Some(m.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Given a raw model name or provider (as a string), returns the first matching model & provider.
    ///
    /// If this is a model and is supported by this node, it is returned directly.
    /// If this is a provider, the first matching model in the node config is returned.
    ///
    /// If there are no matching models with this logic, an error is returned.
    pub fn get_matching_model(&self, model_or_provider: String) -> Result<(ModelProvider, Model)> {
        if let Ok(provider) = ModelProvider::try_from(model_or_provider.clone()) {
            // this is a valid provider, return the first matching model in the config
            self.models
                .iter()
                .find(|(p, _)| *p == provider)
                .ok_or(eyre!(
                    "Provider {} is not supported by this node.",
                    provider
                ))
                .cloned()
        } else if let Ok(model) = Model::try_from(model_or_provider.clone()) {
            // this is a valid model, return it if it is supported by the node
            self.models
                .iter()
                .find(|(_, m)| *m == model)
                .ok_or(eyre!("Model {} is not supported by this node.", model))
                .cloned()
        } else {
            // this is neither a valid provider or model for this node
            Err(eyre!(
                "Given string '{}' is neither a model nor provider.",
                model_or_provider
            ))
        }
    }

    /// From a list of model or provider names, return a random matching model & provider.
    pub fn get_any_matching_model(
        &self,
        list_model_or_provider: Vec<String>,
    ) -> Result<(ModelProvider, Model)> {
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
    pub fn get_providers(&self) -> Vec<ModelProvider> {
        self.models
            .iter()
            .fold(Vec::new(), |mut unique, (provider, _)| {
                if !unique.contains(provider) {
                    unique.push(provider.clone());
                }
                unique
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_parser() {
        let cfg =
            ModelConfig::new_from_csv(Some("idontexist,i dont either,i332287648762".to_string()));
        assert_eq!(cfg.models.len(), 0);

        let cfg = ModelConfig::new_from_csv(Some(
            "gemma2:9b-instruct-q8_0,phi3:14b-medium-4k-instruct-q4_1,balblablabl".to_string(),
        ));
        assert_eq!(cfg.models.len(), 2);
    }

    #[test]
    fn test_model_matching() {
        let cfg = ModelConfig::new_from_csv(Some("gpt-4o,llama3.1:latest".to_string()));
        assert_eq!(
            cfg.get_matching_model("openai".to_string()).unwrap().1,
            Model::GPT4o,
            "Should find existing model"
        );

        assert_eq!(
            cfg.get_matching_model("llama3.1:latest".to_string())
                .unwrap()
                .1,
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
        let cfg = ModelConfig::new_from_csv(Some("gpt-3.5-turbo,llama3.1:latest".to_string()));
        let result = cfg.get_any_matching_model(vec![
            "i-dont-exist".to_string(),
            "llama3.1:latest".to_string(),
            "gpt-4o".to_string(),
            "ollama".to_string(),
        ]);
        assert_eq!(
            result.unwrap().1,
            Model::Llama3_1_8B,
            "Should find existing model"
        );
    }
}
