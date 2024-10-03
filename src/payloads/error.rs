use serde::{Deserialize, Serialize};

/// A task error response.
/// Returning this as the payload helps to debug the errors received at client side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskErrorPayload {
    /// The unique identifier of the task.
    pub task_id: String,
    /// The stringified error object
    pub error: String,
    /// Name of the model that caused the error.
    pub model: String,
}

impl TaskErrorPayload {
    pub fn new(task_id: String, error: String, model: String) -> Self {
        Self {
            task_id,
            error,
            model,
        }
    }
}
