use ollama_workflows::ollama_rs::error::OllamaError;

/// Alias for `Result<T, NodeError>`.
pub type NodeResult<T> = std::result::Result<T, NodeError>;

#[derive(serde::Deserialize)]
pub struct NodeError {
    #[serde(rename = "error")]
    pub message: String,
    pub source: String,
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self) // use same as Debug
    }
}

impl std::fmt::Debug for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} error: {}", self.source, self.message)
    }
}

impl std::error::Error for NodeError {}

impl From<String> for NodeError {
    fn from(message: String) -> Self {
        Self {
            message,
            source: "self".to_string(),
        }
    }
}

impl From<&str> for NodeError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
            source: "self".to_string(),
        }
    }
}

impl From<OllamaError> for NodeError {
    fn from(value: OllamaError) -> Self {
        Self {
            message: value.to_string(),
            source: "ollama-rs".to_string(),
        }
    }
}

impl From<serde_json::Error> for NodeError {
    fn from(value: serde_json::Error) -> Self {
        Self {
            message: value.to_string(),
            source: "serde_json".to_string(),
        }
    }
}

impl From<base64::DecodeError> for NodeError {
    fn from(value: base64::DecodeError) -> Self {
        Self {
            message: value.to_string(),
            source: "base64".to_string(),
        }
    }
}

impl From<hex::FromHexError> for NodeError {
    fn from(value: hex::FromHexError) -> Self {
        Self {
            message: value.to_string(),
            source: "hex".to_string(),
        }
    }
}

impl From<libsecp256k1::Error> for NodeError {
    fn from(value: libsecp256k1::Error) -> Self {
        Self {
            message: value.to_string(),
            source: "secp256k1".to_string(),
        }
    }
}

impl From<libp2p::gossipsub::SubscriptionError> for NodeError {
    fn from(value: libp2p::gossipsub::SubscriptionError) -> Self {
        Self {
            message: value.to_string(),
            source: "gossipsub::subscription".to_string(),
        }
    }
}

impl From<reqwest::Error> for NodeError {
    fn from(value: reqwest::Error) -> Self {
        Self {
            message: value.to_string(),
            source: "reqwest".to_string(),
        }
    }
}

impl From<libp2p::gossipsub::PublishError> for NodeError {
    fn from(value: libp2p::gossipsub::PublishError) -> Self {
        Self {
            message: value.to_string(),
            source: "gossipsub::publish".to_string(),
        }
    }
}
