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
        const TOPIC: &str = "heartbeat";
        let waku = WakuClient::default();
        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);

        waku.relay.subscribe(TOPIC).await.unwrap();

        // get message
        let msgs = waku.relay.get_messages(TOPIC).await.unwrap();
        println!("Messages: {:?}", msgs);
    }
}
