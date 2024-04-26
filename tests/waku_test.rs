#[cfg(feature = "waku_test")]
mod waku_tests {
    use dkn_compute::node::DriaComputeNode;

    #[tokio::test]
    async fn test_base_waku() {
        let waku = DriaComputeNode::default().waku;

        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);

        let peers = waku.peers().await.unwrap();
        assert!(!peers.is_empty());

        let info = waku.info().await.unwrap();
        assert!(!info.listen_addresses.is_empty());
        assert!(info.enr_uri.starts_with("enr:"));
    }

    #[tokio::test]
    async fn test_heartbeat_message() {
        const TOPIC: &str = "heartbeat";
        let waku = DriaComputeNode::default().waku;

        waku.relay.subscribe(TOPIC).await.unwrap();
        waku.relay.get_messages(TOPIC).await.unwrap();
    }
}
