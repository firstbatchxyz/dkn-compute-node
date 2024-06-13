mod mock_messages_test {
    use dkn_compute::{
        compute::payload::{TaskRequestPayload, TaskResponsePayload},
        node::DriaComputeNode,
        utils::crypto::sha256hash,
        waku::message::WakuMessage,
    };
    use fastbloom_rs::{FilterBuilder, Membership};
    use serde::{Deserialize, Serialize};
    use std::env;
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Clone)]
    struct MockPayload {
        number: usize,
    }

    #[tokio::test]
    async fn test_two_tasks() {
        env::set_var("RUST_LOG", "INFO");
        let _ = env_logger::try_init();

        let topic = "testing";
        let input = MockPayload { number: 42 };
        let node = DriaComputeNode::default();
        let mut messages: Vec<WakuMessage> = Vec::new();

        {
            // create filter with your own address
            let mut filter = FilterBuilder::new(128, 0.01).build_bloom_filter();
            filter.add(&node.address());

            let payload = TaskRequestPayload::new(input.clone(), filter);
            let payload_str = serde_json::to_string(&payload).unwrap();

            messages.push(WakuMessage::new(payload_str, topic));
        }

        {
            // create another filter without your own address
            let mut filter = FilterBuilder::new(128, 0.01).build_bloom_filter();
            filter.add(&Uuid::new_v4().to_string().as_bytes()); // something dummy

            let payload = TaskRequestPayload::new(input, filter);
            let payload_str = serde_json::to_string(&payload).unwrap();

            messages.push(WakuMessage::new(payload_str, topic));
        }

        let tasks = node.parse_messages::<MockPayload>(messages);
        assert_eq!(tasks.len(), 1);
    }
}
