use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("config error: {0}")]
    Config(String),

    #[error("identity error: {0}")]
    Identity(String),

    #[error("inference error: {0}")]
    Inference(String),

    #[error("model error: {0}")]
    Model(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("update error: {0}")]
    Update(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<dkn_protocol::ProtocolError> for NodeError {
    fn from(e: dkn_protocol::ProtocolError) -> Self {
        NodeError::Network(e.to_string())
    }
}
