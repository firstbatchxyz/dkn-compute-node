use ollama_workflows::{Model, ModelProvider};

use crate::utils::split_comma_separated;

pub fn parse_models_string(input: Option<String>) -> Vec<(ModelProvider, Model)> {
    let models_str = split_comma_separated(input);
    models_str
        .into_iter()
        .filter_map(|s| match Model::try_from(s) {
            Ok(model) => Some((model.clone().into(), model)),
            Err(e) => {
                log::warn!("Error parsing model: {}", e);
                None
            }
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_0() {
        let models =
            parse_models_string(Some("idontexist,i dont either,i332287648762".to_string()));
        assert_eq!(models.len(), 0);
    }

    #[test]
    fn test_parser_2() {
        let models = parse_models_string(Some(
            "phi3:3.8b,phi3:14b-medium-4k-instruct-q4_1".to_string(),
        ));
        assert_eq!(models.len(), 2);
    }
}
