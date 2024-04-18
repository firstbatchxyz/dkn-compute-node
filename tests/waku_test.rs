#[cfg(feature = "waku-test")]
mod waku_tests {
    use dria_compute_node::clients::waku::WakuClient;

    #[tokio::test]
    async fn test_version() {
        let waku = WakuClient::default();
        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);

        // relayed
        // let msgs = waku
        //     .relay
        //     .get_messages("/dria/1/synthesis/protobuf")
        //     .await
        //     .unwrap();
        // println!("Messages: {:?}", msgs);

        // stored
        // let msgs = waku
        //     .store
        //     .get_messages("/dria/1/synthesis/protobuf", Some(true), None)
        //     .await
        //     .unwrap();
        // println!("Messages: {:?}", msgs);
    }
}
