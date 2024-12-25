use inquire::{required, Text};
use std::env::set_var;

pub fn is_gemini_api_key_required(
    selected_models: &str,
    env_vars: &mut Vec<(String, String)>,
) -> bool {
    // if Gemini model is selected, ask for the api key
    if selected_models.contains("gemini") || selected_models.contains("gemma") {
        let gemini_api_key = Text::new("GEMINI_API_KEY")
            .with_validator(required!("Gemini API key is required"))
            .prompt();

        // set api key in env_vars
        if let Ok(gemini_api_key) = gemini_api_key {
            if let Some((_, new_value)) = env_vars.iter_mut().find(|(k, _)| k == "GEMINI_API_KEY") {
                *new_value = gemini_api_key.clone();
                set_var("GEMINI_API_KEY", gemini_api_key);
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
    #[ignore = "requires Gemini"]
    fn test_is_gemini_api_key_required_with_gpt_model() {
        // create a temp env file
        let temp_file = NamedTempFile::new();
        if let Ok(path) = temp_file {
            let path = path.path().to_path_buf();

            // write test env content
            fs::write(&path, "GEMINI_API_KEY=\n").expect("Unable to write temp file");

            let selected_models = "gemini-1.5-pro,gemma-2-2b-it";
            let mut env_vars = vec![("GEMINI_API_KEY".to_string(), "".to_string())];
            let required = is_gemini_api_key_required(selected_models, &mut env_vars);
            assert!(required);
        }
    }
}
