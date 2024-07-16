use dkn_compute::{
    node::DriaComputeNode,
    utils::crypto::{sha256hash, sign_bytes_recoverable},
    waku::message::P2PMessage,
};
use std::{env, time::Duration};

/// Sends pre-computed signatures on a specific task. This simulates a number of responses to a synthesis task.
#[tokio::test]
#[ignore = "run this manually"]
async fn test_send_multiple_heartbeats() {
    env::set_var("RUST_LOG", "INFO");
    let _ = env_logger::try_init();

    let node = DriaComputeNode::default();
    let timeout = Duration::from_millis(1000);
    let num_heartbeats = 20;

    let uuid = "59b93cb2-5738-4da4-992d-89a1835738d6"; // some random uuid

    let signature = sign_bytes_recoverable(&sha256hash(uuid.as_bytes()), &node.config.secret_key);
    let message = P2PMessage::new(signature, &uuid);

    for i in 1..=num_heartbeats {
        println!("Sending heartbeat #{}", i);
        if let Err(e) = node.send_message_once(message.clone()).await {
            println!("Error sending message: {}", e);
            continue;
        }
        tokio::time::sleep(timeout).await;
    }
}
