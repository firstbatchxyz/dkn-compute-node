use dria_compute_node::{
    config::DriaComputeNodeConfig,
    node::DriaComputeNode,
    workers::{heartbeat::heartbeat_worker, synthesis::synthesis_worker},
};

#[allow(unused)]
use log::{debug, error, info, log_enabled, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let node = DriaComputeNode::new(DriaComputeNodeConfig::new());
    println!("Address: 0x{}", hex::encode(node.address()));

    let mut join_handles = Vec::new();

    join_handles.push(heartbeat_worker(node.clone()));
    join_handles.push(synthesis_worker(node.clone()));

    // await all handles
    for handle in join_handles {
        handle.await.expect("Could not await."); // TODO: handle
    }

    Ok(())
}
