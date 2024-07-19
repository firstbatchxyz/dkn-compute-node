const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
const DEFAULT_OLLAMA_PORT: u16 = 11434;

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub host: String,
    pub port: u16,
    pub hardcoded_models: Vec<String>,
}

impl OllamaConfig {
    /// Looks at the environment variables for Ollama host and port.
    ///
    /// If not found, defaults to `DEFAULT_OLLAMA_HOST` and `DEFAULT_OLLAMA_PORT`.
    pub fn new() -> Self {
        let host = std::env::var("OLLAMA_HOST").unwrap_or(DEFAULT_OLLAMA_HOST.to_string());
        let port = std::env::var("OLLAMA_PORT")
            .and_then(|port_str| {
                port_str
                    .parse::<u16>()
                    .map_err(|_| std::env::VarError::NotPresent)
            })
            .unwrap_or(DEFAULT_OLLAMA_PORT);

        // Ollama workflows may require specific models to be loaded regardless of the choices
        let hardcoded_models = vec!["hellord/mxbai-embed-large-v1:f16"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        OllamaConfig {
            host,
            port,
            hardcoded_models,
        }
    }
}
