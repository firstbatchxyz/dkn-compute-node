use dria_compute_node::{
    config::DriaComputeNodeConfig,
    node::DriaComputeNode,
    utils::{
        crypto::sha256hash,
        message::{self, create_content_topic, WakuMessage},
    },
};
use libsecp256k1::Message;
use tokio::time;

#[allow(unused)]
use log::{debug, error, info, log_enabled, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let node = DriaComputeNode::new(DriaComputeNodeConfig::new());
    println!("Address: 0x{}", hex::encode(node.address()));

    // handle heartbeats
    let mut heartbeat_node = node.clone();
    let heartbeat_handle = tokio::spawn(async move {
        const HEARTBEAT_TOPIC: &str = "heartbeat";

        let topic: String = create_content_topic(HEARTBEAT_TOPIC);
        match heartbeat_node.subscribe_topic(topic.clone()).await {
            Ok(_) => {
                println!("Subscribed to heartbeat topic: {}", topic);
            }
            Err(e) => {
                println!("Error subscribing to heartbeat topic: {:?}", e);
            }
        }
        loop {
            match heartbeat_node
                .process_topic(topic.clone(), |_, messages| {
                    println!("Heartbeats: {:?}", messages);

                    if let Some(message) = messages.last() {
                        let heartbeat_message = WakuMessage::parse(&message.payload);
                        let heartbeat_message = Message::parse(&sha256hash(b"sign-me"));
                        let (signature, recid) = heartbeat_node.sign(&heartbeat_message);
                        let heartbeat_signature = format!(
                            "{}{}",
                            hex::encode(signature.serialize()),
                            hex::encode([recid.serialize()])
                        );
                    }

                    // Some(WakuMessage::new(heartbeat_signature.clone(), &topic, false))
                })
                .await
            {
                Ok(messages_to_publish) => {
                    if let Some(message) = messages_to_publish {
                        println!("Sending heartbeat: {:?}", message);
                    }
                }
                Err(e) => {
                    println!("Error processing heartbeat: {:?}", e);
                }
            }

            time::sleep(time::Duration::from_millis(500)).await;
        }
    });

    // handle synthesis computations
    let mut synthesis_node = node.clone();
    let synthesis_handle = tokio::spawn(async move {
        let topic: String = create_content_topic("synthesis");
        synthesis_node.subscribe_topic(topic.clone()).await;
        loop {
            synthesis_node
                .process_topic(topic.clone(), |_, messages| {
                    println!("Synthesis tasks: {:?}", messages);
                })
                .await;

            time::sleep(time::Duration::from_millis(1000)).await;
        }
    });

    heartbeat_handle.await.unwrap();
    // synthesis_handle.await.unwrap();

    Ok(())
}
