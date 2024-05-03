use dkn_compute::workers::{heartbeat::heartbeat_worker, synthesis::synthesis_worker};
use dkn_compute::{config::DriaComputeNodeConfig, node::DriaComputeNode};
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    let config = DriaComputeNodeConfig::new();
    let node = DriaComputeNode::new(config);

    log::info!("Starting workers");
    let cancellation = CancellationToken::new();
    let tracker = TaskTracker::new();
    tracker.spawn(heartbeat_worker(node.clone(), cancellation.clone()));
    tracker.spawn(synthesis_worker(node.clone(), cancellation.clone()));
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
