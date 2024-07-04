use std::env;
use std::sync::Arc;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use dkn_compute::{
    config::DriaComputeNodeConfig, node::DriaComputeNode, utils::wait_for_termination,
};

use dkn_compute::workers::diagnostic::*;
use dkn_compute::workers::heartbeat::*;
use dkn_compute::workers::workflow::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    log::info!("Using Dria Compute Node v{}", VERSION);

    let config = DriaComputeNodeConfig::new();
    let cancellation = CancellationToken::new();
    let node = Arc::new(DriaComputeNode::new(config, cancellation.clone()));

    log::info!("Checking required services...");
    if let Err(e) = node.check_services().await {
        log::error!("{}", e);
        return Err(e.into());
    }

    // start workers after waiting a bit because Waku takes time to launch
    log::info!("Starting workers in 5 seconds...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    let tracker = TaskTracker::new();

    tracker.spawn(diagnostic_worker(
        node.clone(),
        tokio::time::Duration::from_secs(3),
    ));

    tracker.spawn(heartbeat_worker(
        node.clone(),
        tokio::time::Duration::from_millis(1000),
    ));

    tracker.spawn(workflow_worker(
        node.clone(),
        tokio::time::Duration::from_millis(1000),
    ));

    // close tracker after spawning everything
    tracker.close();

    // wait for all workers
    wait_for_termination(cancellation).await?;
    log::info!("Stopping workers");
    tracker.wait().await;

    Ok(())
}
