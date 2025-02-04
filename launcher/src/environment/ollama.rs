use dotenv_config::EnvConfig;

#[derive(Debug, Default, EnvConfig)]
struct Ollama {
    #[env_config(default = "http://127.0.0.1")]
    host: String,
    #[env_config(default = 11434)]
    port: u16,
    #[env_config(default = true)]
    auto_pull: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_reader() {
        assert!(dotenvy::dotenv().is_ok());
        let cfg = Ollama::init().unwrap();
        println!("{:#?}", cfg);
    }
}
