#[cfg_attr(test, cfg(feature = "waku_test"))]
mod waku_tests {
    use dkn_compute::{node::DriaComputeNode, p2p::P2PMessage};

    #[tokio::test]
    async fn test_base_waku() {
        let waku = DriaComputeNode::default().waku;

        waku.version().await.expect("Should get version");

        let peers = waku.peers().await.expect("Should get peers");
        assert!(!peers.is_empty(), "Expected at least 1 peer");

        let info = waku.info().await.expect("Should get debug info");
        assert!(!info.listen_addresses.is_empty());
        assert!(info.enr_uri.starts_with("enr:"));
    }

    #[tokio::test]
    async fn test_heartbeat_message() {
        const TOPIC: &str = "heartbeat";
        let waku = DriaComputeNode::default().waku;

        waku.relay.subscribe(TOPIC).await.expect("Should subscribe");
        waku.relay
            .get_messages(TOPIC)
            .await
            .expect("Should get messages");
    }

    /// This test sends a message to Waku, sleeps a bit, and then receives it.
    ///
    /// The topic is subscribe at the start, and is unsubscribed at the end.
    #[tokio::test]
    async fn test_message_send_and_receive() {
        let _ = env_logger::try_init();

        let node = DriaComputeNode::default();
        let topic = "test-topic-dria";

        node.subscribe_topic(topic).await;

        // this test checks if we get stuck at the nonce limit of RLNv2
        for i in 1..=20 {
            println!("Sending message #{}", i);
            let message = P2PMessage::new("hello world".to_string(), topic);

            node.send_message(message)
                .await
                .expect("Should send message");

            // wait a bit for the message
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        let messages = node
            .process_topic(topic, false)
            .await
            .expect("Should receive");

        assert!(messages.len() > 0, "Should have received message");

        node.unsubscribe_topic_ignored(topic).await;
    }
}
