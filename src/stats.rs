use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use dkn_protocol::NodeStatsSnapshot;

/// Atomic counters for node-level metrics.
pub struct NodeStats {
    pub tasks_completed: AtomicU64,
    pub tasks_failed: AtomicU64,
    pub tasks_rejected: AtomicU64,
    pub total_tokens_generated: AtomicU64,
    started_at: Instant,
}

impl NodeStats {
    pub fn new() -> Self {
        NodeStats {
            tasks_completed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            tasks_rejected: AtomicU64::new(0),
            total_tokens_generated: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn record_completed(&self, tokens: u32) {
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        self.total_tokens_generated
            .fetch_add(u64::from(tokens), Ordering::Relaxed);
    }

    pub fn record_failed(&self) {
        self.tasks_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_rejected(&self) {
        self.tasks_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn log_summary(&self) {
        let snap = self.snapshot();
        tracing::info!(
            tasks_completed = snap.tasks_completed,
            tasks_failed = snap.tasks_failed,
            tasks_rejected = snap.tasks_rejected,
            total_tokens = snap.total_tokens_generated,
            uptime_secs = snap.uptime_secs,
            "node stats"
        );
    }

    pub fn snapshot(&self) -> NodeStatsSnapshot {
        NodeStatsSnapshot {
            tasks_completed: self.tasks_completed.load(Ordering::Relaxed),
            tasks_failed: self.tasks_failed.load(Ordering::Relaxed),
            tasks_rejected: self.tasks_rejected.load(Ordering::Relaxed),
            total_tokens_generated: self.total_tokens_generated.load(Ordering::Relaxed),
            uptime_secs: self.uptime_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_initial_values() {
        let stats = NodeStats::new();
        assert_eq!(stats.tasks_completed.load(Ordering::Relaxed), 0);
        assert_eq!(stats.tasks_failed.load(Ordering::Relaxed), 0);
        assert_eq!(stats.tasks_rejected.load(Ordering::Relaxed), 0);
        assert_eq!(stats.total_tokens_generated.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_stats_record_completed() {
        let stats = NodeStats::new();
        stats.record_completed(50);
        stats.record_completed(30);
        assert_eq!(stats.tasks_completed.load(Ordering::Relaxed), 2);
        assert_eq!(stats.total_tokens_generated.load(Ordering::Relaxed), 80);
    }

    #[test]
    fn test_stats_record_failed() {
        let stats = NodeStats::new();
        stats.record_failed();
        stats.record_failed();
        assert_eq!(stats.tasks_failed.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_stats_record_rejected() {
        let stats = NodeStats::new();
        stats.record_rejected();
        assert_eq!(stats.tasks_rejected.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_stats_snapshot() {
        let stats = NodeStats::new();
        stats.record_completed(100);
        stats.record_failed();
        stats.record_rejected();
        let snap = stats.snapshot();
        assert_eq!(snap.tasks_completed, 1);
        assert_eq!(snap.tasks_failed, 1);
        assert_eq!(snap.tasks_rejected, 1);
        assert_eq!(snap.total_tokens_generated, 100);
        // uptime_secs should be >= 0 (just created)
        assert!(snap.uptime_secs < 5);
    }
}
