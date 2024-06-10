#[cfg_attr(test, cfg(feature = "heartbeats_test"))]
mod heartbeats_test {
    use dkn_compute::{
        node::DriaComputeNode, utils::crypto::sha256hash, waku::message::WakuMessage,
    };
    use std::{env, time::Duration};

    /// Sends pre-computed signatures on a specific task. This simulates a number of responses to a synthesis task.
    #[tokio::test]
    async fn test_multiple_heartbeats() {
        env::set_var("RUST_LOG", "INFO");
        let _ = env_logger::try_init();

        let node = DriaComputeNode::default();
        let timeout = Duration::from_millis(1000);
        let num_heartbeats = 20;

        let uuid = "59b93cb2-5738-4da4-992d-89a1835738d6"; // some random uuid

        let signature = node.sign_bytes(&sha256hash(uuid.as_bytes()));
        let message = WakuMessage::new(signature, &uuid);

        for i in 1..=num_heartbeats {
            println!("Sending heartbeat #{}", i);
            if let Err(e) = node.send_message_once(message.clone()).await {
                println!("Error sending message: {}", e);
                continue;
            }
            tokio::time::sleep(timeout).await;
        }
    }
}
