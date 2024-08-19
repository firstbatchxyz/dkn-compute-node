use tokio_util::sync::CancellationToken;

use dkn_compute::{DriaComputeNode, DriaComputeNodeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = dotenvy::dotenv() {
        log::warn!("Could not load .env file: {}", e);
    }

    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();
    log::info!(
        "Initializing Dria Compute Node (version {})",
        dkn_compute::VERSION
    );

    // create configurations & check required services
    let config = DriaComputeNodeConfig::new();
    if let Err(err) = config.check_services().await {
        log::error!("Error checking services: {}", err);
        panic!("Service check failed.")
    }

    // launch the node
    let mut node = DriaComputeNode::new(config, CancellationToken::new()).await?;
    if let Err(err) = node.launch().await {
        log::error!("Node error: {}", err);
        panic!("Node failed.")
    };

    Ok(())
}
