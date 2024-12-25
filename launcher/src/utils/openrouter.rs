use inquire::{required, Text};
use std::env::set_var;

pub fn is_openrouter_api_key_required(
    selected_models: &str,
    env_vars: &mut Vec<(String, String)>,
) -> bool {
    // if open router model is selected, ask for the api key
    // TODO: check openrouter model names
    if selected_models.contains("claude")
        || selected_models.contains("qwen")
        || selected_models.contains("deepseek")
        || selected_models.contains("qwq")
    {
        let openrouter_api_key = Text::new("OPENROUTER_API_KEY")
            .with_validator(required!("OpenRouter API key is required"))
            .prompt();

        if let Ok(openrouter_api_key) = openrouter_api_key {
            if let Some((_, new_value)) =
                env_vars.iter_mut().find(|(k, _)| k == "OPENROUTER_API_KEY")
            {
                *new_value = openrouter_api_key.clone();
                set_var("OPENROUTER_API_KEY", openrouter_api_key);
            }
        }
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    #[ignore = "requires Open Router"]
    fn test_is_openai_api_key_required_with_gpt_model() {
        // create a temp env file
        let temp_file = NamedTempFile::new();
        if let Ok(path) = temp_file {
            let path = path.path().to_path_buf();

            // write test env content
            fs::write(&path, "OPENROUTER_API_KEY=\n").expect("Unable to write temp file");

            let selected_models = "qwen-2.5-72b-instruct,qwq-32b-preview";
            let mut env_vars = vec![("OPENROUTER_API_KEY".to_string(), "".to_string())];
            let required = is_openrouter_api_key_required(selected_models, &mut env_vars);
            assert!(required);
        }
    }
}
