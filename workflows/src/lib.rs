mod providers;
pub use providers::OllamaConfig;

mod config;
pub use config::DriaWorkflowsConfig;

/// The body of a task request that includes the workflow and model information.
///
/// Can be used by RPCs, APIs and the compute node.
#[derive(Debug, serde::Deserialize)]
pub struct TaskWorkflow {
    /// [Workflow](https://github.com/andthattoo/ollama-workflows/blob/main/src/program/workflow.rs) object to be parsed.
    pub workflow: Workflow,
    /// A lıst of model (that can be parsed into `Model`) or model provider names.
    /// If model provider is given, the first matching model in the node config is used for that.
    /// From the given list, a random choice will be made for the task.
    pub model: String,
}

// re-export Ollama Workflows
pub use ollama_workflows::*;
