use serde::{Deserialize, Serialize};

/// A generic task request, given by Dria.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestPayload<T> {
    /// The unique identifier of the task.
    pub task_id: String,
    /// The deadline of the task in nanoseconds.
    pub deadline: u128,
    /// The input to the compute function.
    pub input: T,
    /// The public key of the requester, in hexadecimals.
    pub public_key: String,
}
