mod executors;
pub use executors::DriaExecutor;

mod manager;
pub use manager::DriaExecutorsManager;

mod models;
pub use models::{Model, ModelProvider};

mod task;
pub use task::{TaskBody, TaskResult};

pub use rig::completion::CompletionModel;
pub use rig::completion::{CompletionError, PromptError};

// re-export ollama_rs
pub use ollama_rs;
