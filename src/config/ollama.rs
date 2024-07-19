const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
const DEFAULT_OLLAMA_PORT: u16 = 11434;

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub host: String,
    pub port: u16,
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

        OllamaConfig { host, port }
    }
}
