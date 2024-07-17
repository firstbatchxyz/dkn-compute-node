pub mod crypto;
pub mod filter;
pub mod payload;
pub mod provider;

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
