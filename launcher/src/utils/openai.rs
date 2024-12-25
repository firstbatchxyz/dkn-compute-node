use inquire::{required, Text};
use std::env::set_var;

pub fn is_openai_api_key_required(
    selected_models: &str,
    env_vars: &mut Vec<(String, String)>,
) -> bool {
    // if OPENAI model is selected, ask for the api key
    if selected_models.contains("gpt") || selected_models.contains("o1") {
        let openai_api_key = Text::new("OPENAI_API_KEY")
            .with_validator(required!("OpenAI API key is required"))
            .prompt();

        if let Ok(openai_api_key) = openai_api_key {
            if let Some((_, new_value)) = env_vars.iter_mut().find(|(k, _)| k == "OPENAI_API_KEY") {
                *new_value = openai_api_key.clone();
                set_var("OPENAI_API_KEY", openai_api_key);
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
    #[ignore = "requires OpenAI"]
    fn test_is_openai_api_key_required_with_gpt_model() {
        // create a temp env file
        let temp_file = NamedTempFile::new();
        if let Ok(path) = temp_file {
            let path = path.path().to_path_buf();

            // write test env content
            fs::write(&path, "OPENAI_API_KEY=\n").expect("Unable to write temp file");

            let selected_models = "gpt-4o-mini";
            let mut env_vars = vec![("OPENAI_API_KEY".to_string(), "".to_string())];
            let required = is_openai_api_key_required(selected_models, &mut env_vars);
            assert!(required);
        }
    }
}
