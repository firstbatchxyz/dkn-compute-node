use dria_compute_node::{node::DriaComputeNode, utils::message::create_content_topic};

#[allow(unused)]
use log::{debug, error, info, log_enabled, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let mut node = DriaComputeNode::default();
    println!("Address: {:?}", node.address);

    // DKN Heartbeat Handler
    //
    // Dria Node sends heartbeat requests at regular intervals to Waku, and each compute
    // node must respond back to it with a heartbeat response at the respective content topic.
    let heartbeat_handle = tokio::spawn(async move {
        // subscribe to heartbeat topic
        let heartbeat_content_topic = create_content_topic("heartbeat", None);
        node.waku
            .relay
            .subscribe(vec![heartbeat_content_topic.clone()])
            .await
            .expect("Could not subscribe.");

        loop {
            // get latest heartbeat messages
            let messages = node
                .waku
                .relay
                .get_messages(heartbeat_content_topic.as_str())
                .await
                .unwrap();

            // handle each message
            // TODO: !!!
            println!("Messages:\n{:?}", messages);

            // sleep for 5 seconds
            println!("Waiting for a while...");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
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

    heartbeat_handle.await.unwrap();

    Ok(())
}
