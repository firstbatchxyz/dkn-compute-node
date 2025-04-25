mod providers;
pub use providers::OllamaProvider;

mod config;
pub use config::DriaWorkflowsConfig;

mod models;
pub use models::{Model, ModelProvider};

mod task;
pub use task::{TaskBody, TaskResult};

// re-export Ollama Workflows
// pub use ollama_workflows::*;

pub use rig::completion::CompletionModel;
