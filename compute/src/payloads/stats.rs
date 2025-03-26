use serde::{Deserialize, Serialize};

/// Task stats for diagnostics.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats() {
        let mut stats = TaskStats::default();

        assert_eq!(
            stats.received_at,
            chrono::DateTime::<chrono::Utc>::default()
        );
        stats = stats.record_received_at();
        assert_ne!(
            stats.received_at,
            chrono::DateTime::<chrono::Utc>::default()
        );

        assert_eq!(
            stats.published_at,
            chrono::DateTime::<chrono::Utc>::default()
        );
        stats = stats.record_published_at();
        assert_ne!(
            stats.published_at,
            chrono::DateTime::<chrono::Utc>::default()
        );

        println!("{:?}", stats);
    }
}
