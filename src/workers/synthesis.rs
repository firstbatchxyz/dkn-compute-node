use crate::{
    node::DriaComputeNode,
    utils::{
        crypto::sha256hash,
        message::{create_content_topic, WakuMessage},
    },
};
use libsecp256k1::Message;
use serde::{Deserialize, Serialize};

const TOPIC: &str = "synthesis";
const SLEEP_MILLIS: u64 = 500;

// #[derive(Serialize, Deserialize, Debug, Clone)]
// struct SynthesisPayload {
// }

pub fn synthesis_worker(mut node: DriaComputeNode) -> tokio::task::JoinHandle<()> {
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);
    let topic: String = create_content_topic(TOPIC);

    tokio::spawn(async move {
        // subscribe
        match node.subscribe_topic(topic.clone()).await {
            Ok(_) => {
                println!("Subscribed to {}", topic);
            }
            Err(e) => {
                println!("Error subscribing to {}", e);
            }
        }

        loop {
            node.process_topic(topic.clone(), |_, messages| {
                println!("Synthesis tasks: {:?}", messages);
            })
            .await;

            tokio::time::sleep(sleep_amount).await;
        }
    })
}
