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

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
