use dria_compute_node::{config::DriaComputeNodeConfig, node::DriaComputeNode};
use tokio::time;

#[allow(unused)]
use log::{debug, error, info, log_enabled, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // config
    let config = DriaComputeNodeConfig::new();
    let mut node = DriaComputeNode::new(config);
    println!("Address: {:?}", node.address);

    let heartbeat_handle = tokio::spawn(async move {
        node.check_heartbeat().await;
    });

    // DKN Compute Handler
    //
    // Listens to compute requests by Dria Admin Node's (ensured via signatures)
    // tokio::spawn(async move {
    //     loop {
    //         // get latest heartbeat messages
    //         let messages = node.waku.relay.get_messages("fdsf").await.unwrap();

    //         // handle each message
    //         // TODO: !!!

    //         // sleep for 5 seconds
    //         tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    //     }
    // });

    // TODO: sigint / sigterm handling

    heartbeat_handle.await.unwrap();

    Ok(())
}
