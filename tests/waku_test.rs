#[cfg(feature = "waku_test")]
mod waku_tests {
    use dria_compute_node::waku::{message::WakuMessage, WakuClient};

    #[tokio::test]
    async fn test_version() {
        let waku = WakuClient::default();
        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);
    }

    #[tokio::test]
    async fn test_heartbeat_message() {
        let waku = WakuClient::default();
        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);

        // subscribe to content topic message
        let topic = WakuMessage::create_content_topic("heartbeat");
        waku.relay.subscribe(topic.as_str()).await.unwrap();

        // get message
        let msgs = waku
            .store
            .get_messages(&topic, Some(true), None)
            .await
            .unwrap();
        println!("Messages: {:?}", msgs);
    }
}
