use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::utils::get_current_time_nanos;

/// A task stat.
/// Returning this as the payload helps to debug the errors received at client side.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStats {
    /// Timestamp at which the task was received from network & parsed.
    pub received_at: u128,
    /// Timestamp at which the task was published back to network.
    pub published_at: u128,
    /// Time taken to execute the task.
    pub execution_time: u128,
}

impl TaskStats {
    /// Records the current timestamp within `received_at`.
    pub fn record_received_at(mut self) -> Self {
        // can unwrap safely here as UNIX_EPOCH is always smaller than now
        self.received_at = get_current_time_nanos();
        self
    }

    /// Records the current timestamp within `published_at`.
    pub fn record_published_at(mut self) -> Self {
        self.published_at = get_current_time_nanos();
        self
    }

    pub fn record_execution_time(mut self, started_at: Instant) -> Self {
        self.execution_time = Instant::now().duration_since(started_at).as_nanos();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats() {
        let mut stats = TaskStats::default();

        assert_eq!(stats.received_at, 0);
        stats = stats.record_received_at();
        assert_ne!(stats.received_at, 0);

        assert_eq!(stats.published_at, 0);
        stats = stats.record_published_at();
        assert_ne!(stats.published_at, 0);
    }
}
