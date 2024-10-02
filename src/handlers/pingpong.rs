use crate::{
    utils::{get_current_time_nanos, DKNMessage},
    DriaComputeNode,
};
use async_trait::async_trait;
use eyre::Result;
use libp2p::gossipsub::MessageAcceptance;
use ollama_workflows::{Model, ModelProvider};
use serde::{Deserialize, Serialize};

use super::ComputeHandler;

pub struct PingpongHandler;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingpongPayload {
    uuid: String,
    deadline: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingpongResponse {
    pub(crate) uuid: String,
    pub(crate) models: Vec<(ModelProvider, Model)>,
    pub(crate) timestamp: u128,
}

#[async_trait]
impl ComputeHandler for PingpongHandler {
    const LISTEN_TOPIC: &'static str = "ping";
    const RESPONSE_TOPIC: &'static str = "pong";

    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: DKNMessage,
    ) -> Result<MessageAcceptance> {
        let pingpong = message.parse_payload::<PingpongPayload>(true)?;

        // check deadline
        let current_time = get_current_time_nanos();
        if current_time >= pingpong.deadline {
            log::debug!(
                "Ping (uuid: {}) is past the deadline, ignoring. (local: {}, deadline: {})",
                pingpong.uuid,
                current_time,
                pingpong.deadline
            );

            // ignore message due to past deadline
            return Ok(MessageAcceptance::Ignore);
        }

        // respond
        let response_body = PingpongResponse {
            uuid: pingpong.uuid.clone(),
            models: node.config.model_config.models.clone(),
            timestamp: get_current_time_nanos(),
        };

        // publish message
        let message = DKNMessage::new_signed(
            serde_json::json!(response_body).to_string(),
            Self::RESPONSE_TOPIC,
            &node.config.secret_key,
        );
        node.publish(message)?;

        Ok(MessageAcceptance::Accept)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        utils::{
            crypto::{sha256hash, to_address},
            filter::TaskFilter,
            DKNMessage,
        },
        DriaComputeNodeConfig,
    };
    use fastbloom_rs::{FilterBuilder, Membership};
    use libsecp256k1::{recover, Message, PublicKey};

    use super::PingpongPayload;

    #[test]
    fn test_heartbeat_payload() {
        let pk = PublicKey::parse_compressed(&hex_literal::hex!(
            "0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658"
        ))
        .expect("Should parse public key");
        let message = DKNMessage {
            payload: "Y2RmODcyNDlhY2U3YzQ2MDIzYzNkMzBhOTc4ZWY3NjViMWVhZDlmNWJhMDUyY2MxMmY0NzIzMjQyYjc0YmYyODFjMDA1MTdmMGYzM2VkNTgzMzk1YWUzMTY1ODQ3NWQyNDRlODAxYzAxZDE5MjYwMDM1MTRkNzEwMThmYTJkNjEwMXsidXVpZCI6ICI4MWE2M2EzNC05NmM2LTRlNWEtOTliNS02YjI3NGQ5ZGUxNzUiLCAiZGVhZGxpbmUiOiAxNzE0MTI4NzkyfQ==".to_string(),
            topic: "heartbeat".to_string(),
            version: "0.0.0".to_string(),
            timestamp: 1714129073557846272,
        };

        assert!(message.is_signed(&pk).expect("Should check signature"));

        let obj = message
            .parse_payload::<PingpongPayload>(true)
            .expect("Should parse payload");
        assert_eq!(obj.uuid, "81a63a34-96c6-4e5a-99b5-6b274d9de175");
        assert_eq!(obj.deadline, 1714128792);
    }

    /// This test demonstrates the process of heartbeat & task assignment.
    ///
    /// A heart-beat message is sent over the network by Admin Node, and compute node responds with a signature.
    #[test]
    fn test_heartbeat_and_task_assignment() {
        let config = DriaComputeNodeConfig::default();

        // a heartbeat message is signed and sent to Admin Node over the p2p network
        let heartbeat_message = Message::parse(&sha256hash(b"sign-me"));
        let (heartbeat_signature, heartbeat_recid) =
            libsecp256k1::sign(&heartbeat_message, &config.secret_key);

        // admin recovers the address from the signature
        let recovered_public_key =
            recover(&heartbeat_message, &heartbeat_signature, &heartbeat_recid)
                .expect("Could not recover");
        assert_eq!(
            config.public_key, recovered_public_key,
            "Public key mismatch"
        );
        let address = to_address(&recovered_public_key);
        assert_eq!(address, config.address, "Address mismatch");

        // admin node assigns the task to the compute node via Bloom Filter
        let mut bloom = FilterBuilder::new(100, 0.01).build_bloom_filter();
        bloom.add(&address);
        let filter_payload = TaskFilter::from(bloom);

        // compute node receives the filter and checks if it is tasked
        assert!(
            filter_payload
                .contains(&config.address)
                .expect("Should check filter"),
            "Node should be tasked"
        );
    }
}
