use serde::Deserialize;

pub type NodeResult<T> = std::result::Result<T, NodeError>;

#[derive(Deserialize)]
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

impl From<reqwest::Error> for NodeError {
    fn from(value: reqwest::Error) -> Self {
        Self {
            message: value.to_string(),
            source: "reqwest".to_string(),
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
