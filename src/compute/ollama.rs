use std::{borrow::Borrow, env};

use ollama_rs::{
    error::OllamaError,
    generation::completion::{request::GenerationRequest, GenerationResponse},
    models::pull::PullModelStatus,
    Ollama,
};

pub const DEFAULT_DKN_OLLAMA_HOST: &str = "http://127.0.0.1";
pub const DEFAULT_DKN_OLLAMA_PORT: u16 = 11434;

/// A wrapper for the Ollama API.
#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub client: Ollama,
    pub model: OllamaModel,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new(
            Some(DEFAULT_DKN_OLLAMA_HOST.to_string()),
            Some(DEFAULT_DKN_OLLAMA_PORT),
            Some(OllamaModel::default()),
        )
    }
}

impl OllamaClient {
    /// Creates a new Ollama client.
    ///
    /// Reads `DKN_OLLAMA_HOST` and `DKN_OLLAMA_PORT` from the environment, and defaults if not provided.
    pub fn new(host: Option<String>, port: Option<u16>, model: Option<OllamaModel>) -> Self {
        let host = host.unwrap_or_else(|| {
            env::var("DKN_OLLAMA_HOST").unwrap_or(DEFAULT_DKN_OLLAMA_HOST.to_string())
        });

        let port = port.unwrap_or_else(|| {
            env::var("DKN_OLLAMA_PORT")
                .and_then(|port_str| {
                    port_str
                        .parse::<u16>()
                        .map_err(|_| env::VarError::NotPresent)
                })
                .unwrap_or(DEFAULT_DKN_OLLAMA_PORT)
        });

        let model = model.unwrap_or_else(|| {
            OllamaModel::from(
                env::var("DKN_OLLAMA_MODEL").unwrap_or(String::from(&OllamaModel::default())),
            )
        });

        Self {
            client: Ollama::new(host, port),
            model,
        }
    }

    /// Pulls the configured model.
    pub async fn setup(&self) -> Result<PullModelStatus, OllamaError> {
        log::info!("Pulling model: {:?}", self.model);
        self.client
            .pull_model(self.model.borrow().into(), false)
            .await
    }

    /// Generates a result using the local LLM.
    pub async fn generate(&self, prompt: String) -> Result<GenerationResponse, String> {
        self.client
            .generate(GenerationRequest::new(self.model.borrow().into(), prompt))
            .await
    }
}

#[allow(non_camel_case_types)]
#[derive(Default, Clone, Debug, PartialEq)]
pub enum OllamaModel {
    #[default] ///////// Param  Memory  Command
    Mistral, /////////// 7B     4.1GB   ollama run mistral
    Llama2Uncensored, // 7B	    3.8GB	ollama run llama2-uncensored
    Llama2_13B,       // 13B    7.3GB   ollama run llama2:13b
    Llama2_70B,       // 70B    39GB	ollama run llama2:70b
    Llama3_8B,        // 8B	    4.7GB	ollama run llama3
    Llama3_70B,       // 70B    40GB	ollama run llama3:70b
    DolphinPhi,       // 2.7B   1.6GB	ollama run dolphin-phi
    Phi2,             // 2.7B   1.7GB	ollama run phi
    NeuralChat,       // 7B	    4.1GB	ollama run neural-chat
    Starling,         // 7B	    4.1GB	ollama run starling-lm
    CodeLlama,        // 7B	    3.8GB	ollama run codellama
    OrcaMini,         // 3B	    1.9GB	ollama run orca-mini
    LLaVA,            // 7B	    4.5GB	ollama run llava
    Gemma_2B,         // 2B	    1.4GB	ollama run gemma:2b
    Gemma_7B,         // 7B	    4.8GB	ollama run gemma:7b
    Solar,            // 10.7B  6.1GB	ollama run solar
}

impl From<&OllamaModel> for String {
    /// Returns the model `name` such that it can be used as `ollama run <name>`.
    fn from(value: &OllamaModel) -> Self {
        match value {
            OllamaModel::Llama3_8B => "llama3",
            OllamaModel::Llama3_70B => "llama3:70b",
            OllamaModel::Mistral => "mistral",
            OllamaModel::DolphinPhi => "dolphin-phi",
            OllamaModel::Phi2 => "phi",
            OllamaModel::NeuralChat => "neural-chat",
            OllamaModel::Starling => "starling-lm",
            OllamaModel::CodeLlama => "codellama",
            OllamaModel::Llama2Uncensored => "llama2-uncensored",
            OllamaModel::Llama2_13B => "llama2:13b",
            OllamaModel::Llama2_70B => "llama2:70b",
            OllamaModel::OrcaMini => "orca-mini",
            OllamaModel::LLaVA => "llava",
            OllamaModel::Gemma_2B => "gemma:2b",
            OllamaModel::Gemma_7B => "gemma:7b",
            OllamaModel::Solar => "solar",
        }
        .to_string()
    }
}

impl From<String> for OllamaModel {
    fn from(value: String) -> Self {
        match value.as_str() {
            "llama3" => OllamaModel::Llama3_8B,
            "llama3:70b" => OllamaModel::Llama3_70B,
            "mistral" => OllamaModel::Mistral,
            "dolphin-phi" => OllamaModel::DolphinPhi,
            "phi" => OllamaModel::Phi2,
            "neural-chat" => OllamaModel::NeuralChat,
            "starling-lm" => OllamaModel::Starling,
            "codellama" => OllamaModel::CodeLlama,
            "llama2-uncensored" => OllamaModel::Llama2Uncensored,
            "llama2:13b" => OllamaModel::Llama2_13B,
            "llama2:70b" => OllamaModel::Llama2_70B,
            "orca-mini" => OllamaModel::OrcaMini,
            "llava" => OllamaModel::LLaVA,
            "gemma:2b" => OllamaModel::Gemma_2B,
            "gemma:7b" => OllamaModel::Gemma_7B,
            "solar" => OllamaModel::Solar,
            _ => {
                log::warn!("Unknown model: {}, using default.", value);
                OllamaModel::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_config() {
        env::set_var("DKN_OLLAMA_HOST", "im-a-host");
        env::set_var("DKN_OLLAMA_MODEL", "phi");
        env::remove_var("DKN_OLLAMA_PORT");

        // will use default port, but read host and model from env
        let ollama = OllamaClient::new(None, None, None);
        assert_eq!(ollama.client.uri(), "im-a-host:11434");
        assert_eq!(ollama.model, OllamaModel::Phi2);
    }
}
