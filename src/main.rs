use dria_compute_node::{
    config::DriaComputeNodeConfig, node::DriaComputeNode, workers::heartbeat::heartbeat_worker,
};

#[allow(unused)]
use log::{debug, error, info, log_enabled, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let node = DriaComputeNode::new(DriaComputeNodeConfig::new());
    println!("Address: 0x{}", hex::encode(node.address()));

    let heartbeat_handle = heartbeat_worker(node.clone());
    let synthesis_handle = heartbeat_handle.await.unwrap();
    // synthesis_handle.await.unwrap();

    Ok(())
}
