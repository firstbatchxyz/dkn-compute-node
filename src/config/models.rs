use ollama_workflows::{Model, ModelProvider};

use crate::utils::split_comma_separated;

pub fn parse_models_string(input: Option<String>) -> Vec<(ModelProvider, Model)> {
    let models_str = split_comma_separated(input);
    let providers_models = models_str
        .into_iter()
        .filter_map(|s| match Model::try_from(s) {
            Ok(model) => Some((model.clone().into(), model)),
            Err(e) => {
                log::warn!("Error parsing model: {}", e);
                None
            }
        })
        .collect::<Vec<_>>();

    if providers_models.is_empty() {
        log::error!("No models were provided, using the default model instead.");
        log::error!("Make sure to restart with at least one model provided within DKN_MODELS.");

        vec![(ModelProvider::OpenAI, Model::GPT3_5Turbo)]
    } else {
        providers_models
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_default() {
        let models =
            parse_models_string(Some("idontexist,i dont either,i332287648762".to_string()));
        assert_eq!(models.len(), 1);
        assert!(models.contains(&(ModelProvider::OpenAI, Model::GPT3_5Turbo)));
    }

    #[test]
    fn test_parser_2_models() {
        let models = parse_models_string(Some(
            "phi3:3.8b,phi3:14b-medium-4k-instruct-q4_1".to_string(),
        ));
        assert_eq!(models.len(), 2);
    }
}
