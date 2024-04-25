use crate::{node::DriaComputeNode, utils::crypto::sha256hash, waku::message::WakuMessage};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

const TOPIC: &str = "heartbeat";
const SLEEP_MILLIS: u64 = 500;

/// Heartbeat Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String,
    deadline: u128,
}

pub fn heartbeat_worker(
    mut node: DriaComputeNode,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    let sleep_amount = tokio::time::Duration::from_millis(SLEEP_MILLIS);

    tokio::spawn(async move {
        match node.subscribe_topic(TOPIC).await {
            Ok(_) => {
                println!("Subscribed to {}", TOPIC);
            }
            Err(e) => {
                println!("Error subscribing to {}", e);
            }
        }

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => { break; }
                _ = tokio::time::sleep(sleep_amount) => {
                    let mut msg_to_send: Option<WakuMessage> = None;
                    if let Ok(messages) = node.process_topic(TOPIC).await {
                        // println!("Heartbeats: {:?}", messages);

                        // we only care about the latest heartbeat
                        if let Some(message) = messages.last() {
                            println!("HB MESSAGE: {:?}", message);

                            let uuid = message
                                .parse_payload::<HeartbeatPayload>()
                                .expect("TODO TODO") // TODO: error handling
                                .uuid;
                            let signature = node.sign_bytes(&sha256hash(uuid.as_bytes()));

                            msg_to_send = Some(WakuMessage::new(signature, &uuid));
                        }
                    }

                    // send message
                    if let Some(message) = msg_to_send {
                        node.waku
                            .relay
                            .send_message(message)
                            .await
                            .expect("TODO TODO");
                    }
                }
            }

            // tokio::time::sleep(sleep_amount).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use ecies::PublicKey;
    use libsecp256k1::PublicKeyFormat;

    use crate::{
        config::defaults::DEFAULT_DKN_ADMIN_PUBLIC_KEY, waku::message::WakuMessage,
        workers::heartbeat::HeartbeatPayload,
    };

    #[test]
    fn test_raw_heartbeat() {
        let message = serde_json::from_str::<WakuMessage>("{ \"payload\": \"ODI3NTEzYzU4NDIzNWI2ZDQ3MTAwZDUxOTViMTc2ZDk3MTNlZTMyOGU0ZmQ5Yjg2ODU0OTBhYTViNTZmNDVmNDM5OTkwNTg4MTU4YTU1YzFhMDRiNjVhMTEyZDJlZTQxNWMyMzllNjg4ZGViMDY3NmMwYWU2NjU3ZmM0ODlmZWYwMHsidXVpZCI6ICIxMjg5MjZjZC05NGEyLTQxNjMtYWVjMC1mNTIyZDZlMjA2N2MifQ==\", \"contentTopic\": \"/dria/0/heartbeat/proto\"}").expect("Could not parse");
        let public_key = PublicKey::parse_slice(
            hex::decode(DEFAULT_DKN_ADMIN_PUBLIC_KEY.to_string())
                .unwrap()
                .as_slice(),
            Some(PublicKeyFormat::Compressed),
        )
        .unwrap();

        let parsed = message.parse_signed_payload::<HeartbeatPayload>(&public_key);
        assert!(parsed.is_ok());
    }
}
