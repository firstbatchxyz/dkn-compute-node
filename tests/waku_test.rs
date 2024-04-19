// #[cfg(feature = "waku_test")]
mod waku_tests {
    use dria_compute_node::{utils::message::create_content_topic, waku::WakuClient};

    #[tokio::test]
    async fn test_version() {
        let mut waku = WakuClient::default();
        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);

        // subscribe to content topic message
        let topic = create_content_topic("heartbeat", None);
        waku.relay.subscribe(vec![topic.clone()]).await.unwrap();

        // get message
        let msgs = waku
            .store
            .get_messages(&topic, Some(true), None)
            .await
            .unwrap();
        println!("Messages: {:?}", msgs);
    }
}
