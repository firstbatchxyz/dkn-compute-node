pub mod crypto;
pub mod filter;
pub mod payload;

mod message;
pub use message::P2PMessage;

mod available_nodes;
pub use available_nodes::AvailableNodes;

use std::time::{Duration, SystemTime};

/// Returns the current time in nanoseconds since the Unix epoch.
///
/// If a `SystemTimeError` occurs, will return 0 just to keep things running.
#[inline]
pub fn get_current_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|e| {
            log::error!("Error getting current time: {}", e);
            Duration::new(0, 0)
        })
        .as_nanos()
}

/// Utility to parse comma-separated string values, mostly read from the environment.
/// - Trims `"` from both ends at the start
/// - For each item, trims whitespace from both ends
pub fn split_comma_separated(input: Option<String>) -> Vec<String> {
    match input {
        Some(s) => s
            .trim_matches('"')
            .split(',')
            .filter_map(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.trim().to_string())
                }
            })
            .collect::<Vec<_>>(),
        None => vec![],
    }
}
