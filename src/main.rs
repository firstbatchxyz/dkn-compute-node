use dria_compute_node::{
    config::DriaComputeNodeConfig,
    node::DriaComputeNode,
    workers::{heartbeat::heartbeat_worker, synthesis::synthesis_worker},
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let node = DriaComputeNode::new(DriaComputeNodeConfig::new());
    log::info!("Address: 0x{}", hex::encode(node.address()));

    let cancellation = CancellationToken::new();

    let mut join_handles = Vec::new();
    join_handles.push(heartbeat_worker(node.clone(), cancellation.clone()));
    join_handles.push(synthesis_worker(node.clone(), cancellation.clone()));

    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            println!("Received CTRL+C, stopping workers.");
            cancellation.cancel();
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }

    for handle in join_handles {
        handle.await.expect("Could not await."); // TODO: handle
    }

    Ok(())
}
