use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Topic used within [`crate::DriaMessage`] for task request messages.
pub const TASK_REQUEST_TOPIC: &str = "task";

/// Topic used within [`crate::DriaMessage`] for task result messages.
pub const TASK_RESULT_TOPIC: &str = "results";

/// A computation task is the task of computing a result from a given input.
///
/// `result` and `error` are mutually-exclusive, only one of them can be `Some`:
/// - if `result` is `Some`, then it contains the result.
/// - if `error` is `Some`, then it contains the error message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponsePayload {
    /// The uprimary key of the row in the database for this task.
    pub row_id: Uuid,
    /// The unique identifier of the task.
    pub task_id: Uuid,
    /// Name of the model used for this task.
    pub model: String,
    /// Stats about the task execution.
    pub stats: TaskStats,
    /// Result from the LLM, as-is.
    ///
    /// If this is `None`, the task failed, and you should check the `error` field.
    pub result: Option<String>,
    /// An error message, if any.
    ///
    /// If this is `Some`, you can ignore the `result` field.
    pub error: Option<String>,
}

impl TaskResponsePayload {
    /// Creates the payload of a computation with its result.
    pub fn new(
        result: String,
        row_id: Uuid,
        task_id: Uuid,
        model: String,
        stats: TaskStats,
    ) -> Result<Self, libsecp256k1::Error> {
        Ok(TaskResponsePayload {
            row_id,
            task_id,
            result: Some(result),
            model,
            stats,
            error: None,
        })
    }

    /// Creates the payload of a computation with an error message.
    pub fn new_error(
        error: String,
        row_id: Uuid,
        task_id: Uuid,
        model: String,
        stats: TaskStats,
    ) -> Self {
        TaskResponsePayload {
            row_id,
            task_id,
            result: None,
            model,
            stats,
            error: Some(error),
        }
    }
}

/// A generic task request, given by Dria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequestPayload<T> {
    /// The uprimary key of the row in the database for this task.
    ///
    /// Can be both snake case or camel case.
    #[serde(alias = "rowId")]
    pub row_id: Uuid,
    /// The unique identifier of the task.
    ///
    /// Can be both snake case or camel case.
    #[serde(alias = "taskId")]
    pub task_id: Uuid,
    /// The input to the compute function.
    pub input: T,
}

/// Task stats for diagnostics.
///
/// Returning this as the payload helps to debug the errors received at client side, and latencies.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStats {
    /// Timestamp at which the task was received from network & parsed.
    pub received_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp at which the task was published back to network.
    pub published_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp at which the task execution had started.
    pub execution_started_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp at which the task execution had finished.
    pub execution_ended_at: chrono::DateTime<chrono::Utc>,
    /// Number of tokens of the result.
    pub token_count: usize,
}

impl TaskStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the current timestamp within `received_at`.
    pub fn record_received_at(mut self) -> Self {
        self.received_at = chrono::Utc::now();
        self
    }

    /// Records the current timestamp within `published_at`.
    pub fn record_published_at(mut self) -> Self {
        self.published_at = chrono::Utc::now();
        self
    }

    /// Records the execution start time within `execution_started_at`.
    pub fn record_execution_started_at(mut self) -> Self {
        self.execution_started_at = chrono::Utc::now();
        self
    }

    /// Records the execution end time within `execution_ended_time`.
    pub fn record_execution_ended_at(mut self) -> Self {
        self.execution_ended_at = chrono::Utc::now();
        self
    }

    /// Records the token count within `token_count`.
    pub fn record_token_count(mut self, token_count: usize) -> Self {
        self.token_count = token_count;
        self
    }
}
