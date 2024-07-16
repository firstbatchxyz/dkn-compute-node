use std::env;
use tokio_util::sync::CancellationToken;

use dkn_compute::{
    config::DriaComputeNodeConfig, node::DriaComputeNode, utils::wait_for_termination,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    log::info!("Using Dria Compute Node v{}", VERSION);

    let config = DriaComputeNodeConfig::new();
    let cancellation = CancellationToken::new();

    log::info!("Initializing Dria Compute Node...");
    let mut node = DriaComputeNode::new(config, cancellation.clone())?;

    // start handling tasks
    node.launch().await;

    // wait for all workers
    wait_for_termination(cancellation).await?;
    log::info!("Terminating the node...");

    Ok(())
}
