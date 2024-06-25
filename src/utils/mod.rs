pub mod crypto;
pub mod filter;
pub mod http;
pub mod payload;
pub mod provider;

use std::time::{Duration, SystemTime};
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;

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

/// Waits for SIGTERM or SIGINT, and cancels the given token when the signal is received.
pub async fn wait_for_termination(cancellation: CancellationToken) -> std::io::Result<()> {
    let mut sigterm = signal(SignalKind::terminate())?; // Docker sends SIGTERM
    let mut sigint = signal(SignalKind::interrupt())?; // Ctrl+C sends SIGINT
    tokio::select! {
        _ = sigterm.recv() => log::warn!("Recieved SIGTERM"),
        _ = sigint.recv() => log::warn!("Recieved SIGINT"),
    };

    cancellation.cancel();
    Ok(())
}
