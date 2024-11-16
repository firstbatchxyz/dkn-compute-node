use serde::{Deserialize, Serialize};

use super::TaskStats;

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
    /// Task statistics.
    pub stats: TaskStats,
}
