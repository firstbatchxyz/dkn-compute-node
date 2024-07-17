use tokio_util::sync::CancellationToken;

use dkn_compute::{DriaComputeNode, DriaComputeNodeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    log::info!("Initializing Dria Compute Node {}...", dkn_compute::VERSION);
    let config = DriaComputeNodeConfig::new();
    let cancellation = CancellationToken::new();
    let mut node = DriaComputeNode::new(config, cancellation.clone())?;
    node.check_services().await?;
    node.launch().await?;

    Ok(())
}
