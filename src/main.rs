#[allow(unused)]
use dkn_compute::workers::{heartbeat::heartbeat_worker, synthesis::synthesis_worker};
use dkn_compute::{config::DriaComputeNodeConfig, node::DriaComputeNode};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        // setting this to None disables the timestamp
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    let node = DriaComputeNode::new(DriaComputeNodeConfig::new());
    log::info!("Address:    0x{}", hex::encode(node.address()));
    log::info!(
        "Public Key: 0x{}",
        hex::encode(node.config.DKN_WALLET_PUBLIC_KEY.serialize_compressed())
    );

    let cancellation = CancellationToken::new();
    let mut join_handles = Vec::new();

    #[cfg(feature = "heartbeat")]
    join_handles.push(heartbeat_worker(node.clone(), cancellation.clone()));

    #[cfg(feature = "synthesis")]
    join_handles.push(synthesis_worker(node.clone(), cancellation.clone()));

    // SIGINT handler
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            log::warn!("Received SIGINT, stopping workers.");
            cancellation.cancel();
        }
        Err(err) => {
            log::error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // wait for all workers
    for handle in join_handles {
        handle.await.expect("Could not await."); // TODO: handle
    }

    Ok(())
}
