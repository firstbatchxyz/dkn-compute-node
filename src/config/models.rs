use ollama_workflows::Model;

pub fn parse_dkn_models(models_str: String) -> Vec<Model> {
    let tasks: Vec<Model> = models_str
        .split(',')
        .filter_map(|s| {
            let s = s.trim().to_lowercase();
            match Model::try_from(s) {
                Ok(model) => Some(model),
                Err(e) => {
                    log::warn!("Invalid model: {}", e);
                    None
                }
            }
        })
        .collect();

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_1() {
        let models = parse_dkn_models("idontexist,i dont either,i332287648762".to_string());
        assert_eq!(models.len(), 0);
    }

    #[test]
    fn test_parser_2() {
        let models = parse_dkn_models("phi3:3.8b,phi3:14b-medium-4k-instruct-q4_1".to_string());
        assert_eq!(models.len(), 2);
    }
}
