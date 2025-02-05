use dotenv_config::EnvConfig;

const API_KEY_NAMES: [&'static str; 5] = [
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "OPENROUTER_API_KEY",
    "SERPER_API_KEY",
    "JINA_API_KEY",
];

#[derive(Debug, Default, EnvConfig)]
pub struct ApiKeys {
    #[env_config(name = "OPENAI_API_KEY")]
    pub openai_api_key: String,
    #[env_config(name = "GEMINI_API_KEY")]
    pub gemini_api_key: String,
    #[env_config(name = "OPENROUTER_API_KEY")]
    pub openrouter_api_key: String,
    #[env_config(name = "SERPER_API_KEY")]
    pub serper_api_key: String,
    #[env_config(name = "JINA_API_KEY")]
    pub jina_api_key: String,
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_env_reader() {
        let old_serper = env::var("SERPER_API_KEY");
        let old_openai = env::var("OPENAI_API_KEY");

        env::set_var("SERPER_API_KEY", "imserper");
        env::remove_var("OPENAI_API_KEY");
        let cfg = ApiKeys::init().unwrap();
        assert!(cfg.serper_api_key == "imserper");
        assert!(cfg.openai_api_key == "");

        if let Ok(val) = old_serper {
            env::set_var("SERPER_API_KEY", val);
        }
        if let Ok(val) = old_openai {
            env::set_var("OPENAI_API_KEY", val);
        }
    }
}
