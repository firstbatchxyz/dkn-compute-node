use dkn_compute::workers::heartbeat::*;
use dkn_compute::{config::DriaComputeNodeConfig, node::DriaComputeNode};
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

#[cfg(feature = "synthesis")]
use dkn_compute::workers::synthesis::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    log::info!("Using Dria Compute Node v{}", VERSION);

    let config = DriaComputeNodeConfig::new();
    let node = DriaComputeNode::new(config);

    log::info!("Starting workers");
    let cancellation = CancellationToken::new();
    let tracker = TaskTracker::new();

    // heartbeat is always enabled
    tracker.spawn(heartbeat_worker(
        node.clone(),
        cancellation.clone(),
        "heartbeat",
        tokio::time::Duration::from_millis(1000),
    ));

    #[cfg(feature = "synthesis")]
    tracker.spawn(synthesis_worker(
        node.clone(),
        cancellation.clone(),
        "synthesis",
        tokio::time::Duration::from_millis(1000),
    ));

    tracker.close(); // close tracker after spawning everything

    // wait for termination signals
    let mut sigterm = signal(SignalKind::terminate())?; // Docker sends SIGTERM
    let mut sigint = signal(SignalKind::interrupt())?; // Ctrl+C sends SIGINT
    tokio::select! {
        _ = sigterm.recv() => log::warn!("Recieved SIGTERM"),
        _ = sigint.recv() => log::warn!("Recieved SIGINT"),
    };

    // cancel all workers
    cancellation.cancel();

    // wait for all workers
    log::warn!("Stopping workers");
    tracker.wait().await;

    Ok(())
}
