use crate::{
    node::DriaComputeNode,
    utils::{
        crypto::sha256hash,
        message::{create_content_topic, WakuMessage},
    },
};
use libsecp256k1::Message;
use serde::{Deserialize, Serialize};

const TOPIC: &str = "heartbeat";
const SLEEP_MILLIS: u64 = 500;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String, // TODO: format?
}

pub fn heartbeat_worker(mut node: DriaComputeNode) -> tokio::task::JoinHandle<()> {
    let topic: String = create_content_topic(TOPIC);
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);

    tokio::spawn(async move {
        match node.subscribe_topic(topic.clone()).await {
            Ok(_) => {
                println!("Subscribed to {}", topic);
            }
            Err(e) => {
                println!("Error subscribing to {}", e);
            }
        }

        // handle heartbeat messages in set intervals
        loop {
            match node
                .process_topic(topic.clone(), |_, messages| {
                    println!("Heartbeats: {:?}", messages);

                    // get the last message that is authentic
                    if let Some(message) = messages.last() {
                        // decode the payload and sign it yourself
                        let heartbeat_payload = message.decode_payload().expect("TODO TODO");
                        let digest = sha256hash(heartbeat_payload.as_slice());
                        let (signature, recid) = node.sign(&Message::parse(&digest));
                        let heartbeat_signature = format!(
                            "{}{}",
                            hex::encode(signature.serialize()),
                            hex::encode([recid.serialize()])
                        );

                        // TODO: set content topic to be <uuid>
                        Some(WakuMessage::new(heartbeat_signature, &topic, false))
                    } else {
                        None
                    }
                })
                .await
            {
                Ok(messages_to_publish) => {
                    if let Some(message) = messages_to_publish {
                        println!("Sending heartbeat messages: {:?}", message);
                    }
                }
                Err(e) => {
                    println!("Error processing heartbeat: {:?}", e);
                }
            }

            tokio::time::sleep(sleep_amount).await;
        }
    })
}
