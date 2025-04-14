// TODO: with each successfull heartbeat

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartbeatRequest {
    /// A unique ID for the heartbeat request.
    pub heartbeat_id: Uuid,
    /// Deadline for the heartbeat request, in nanoseconds.
    pub deadline: chrono::DateTime<chrono::Utc>,
    /// Model names available in the node.
    pub models: Vec<String>,
    /// Number of tasks in the channel currently, `single` and `batch`.
    pub pending_tasks: [usize; 2],
}

/// The response is an object with UUID along with an ACK (acknowledgement).
///
/// If for any reason the `error` is `Some`, the request is considered failed.
/// This may be when `deadline` is past the current time, or if the node is deeming itself unhealthy.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartbeatResponse {
    /// UUID as given in the request.
    pub heartbeat_id: Uuid,
    /// An associated error with the response:
    /// - `None` means that the heartbeat was acknowledged.
    /// - `Some` means that the heartbeat was not acknowledged for the given reason.
    pub error: Option<String>,
}
