use std::env;
use tokio_util::sync::CancellationToken;

use dkn_compute::{config::DriaComputeNodeConfig, node::DriaComputeNode};

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
    node.check_services().await?;
    node.launch().await?;

    Ok(())
}
