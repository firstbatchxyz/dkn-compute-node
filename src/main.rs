use dria_compute_node::{
    config::DriaComputeNodeConfig, node::DriaComputeNode, utils::message::create_content_topic,
};
use tokio::time;

#[allow(unused)]
use log::{debug, error, info, log_enabled, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // setup node
    let config = DriaComputeNodeConfig::new();
    let node = DriaComputeNode::new(config.clone());
    println!("Address: 0x{}", hex::encode(node.address));

    // handle heartbeats
    let mut heartbeat_node = node.clone();
    let heartbeat_handle = tokio::spawn(async move {
        let topic: String = create_content_topic("heartbeat");
        heartbeat_node.subscribe_topic(topic.clone()).await;
        loop {
            heartbeat_node
                .process_topic(topic.clone(), |_, messages| {
                    println!("Received heartbeats: {:?}", messages);
                })
                .await;

            time::sleep(time::Duration::from_millis(500)).await;
        }
    });

    // handle synthesis computations
    // let mut synthesis_node = node.clone();
    // let synthesis_handle = tokio::spawn(async move {
    //     let topic: String = create_content_topic("synthesis");
    //     synthesis_node.subscribe_topic(topic.clone()).await;
    //     loop {
    //         synthesis_node
    //             .process_topic(topic.clone(), |_, m| {
    //                 println!("Received heartbeat: {:?}", m);
    //             })
    //             .await;

    //         time::sleep(time::Duration::from_millis(1000)).await;
    //     }
    // });

    // handle SIGTERM
    // tokio::signal::ctrl_c().await.unwrap();
    heartbeat_handle.await.unwrap();
    // synthesis_handle.await.unwrap();

    Ok(())
}
