use crate::{
    errors::NodeResult, node::DriaComputeNode, p2p::P2PMessage, utils::get_current_time_nanos,
};
use ollama_workflows::{Model, ModelProvider};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingpongPayload {
    uuid: String,
    deadline: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingpongResponse {
    pub(crate) uuid: String,
    pub(crate) models: Vec<(ModelProvider, Model)>,
}

/// A ping-pong is a message sent by a node to indicate that it is alive.
/// Compute nodes listen to `pong` topic, and respond to `ping` topic.
pub trait HandlesPingpong {
    fn handle_heartbeat(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()>;
}

impl HandlesPingpong for DriaComputeNode {
    fn handle_heartbeat(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()> {
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
            return Ok(());
        }

        // respond
        let response_body = PingpongResponse {
            uuid: pingpong.uuid.clone(),
            models: self.config.model_config.models.clone(),
        };
        let response = P2PMessage::new_signed(
            serde_json::json!(response_body).to_string(),
            result_topic,
            &self.config.secret_key,
        );
        self.publish(response)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        p2p::P2PMessage,
        utils::{
            crypto::{sha256hash, to_address},
            filter::FilterPayload,
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
        let message = P2PMessage {
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
        let filter_payload = FilterPayload::from(bloom);

        // compute node receives the filter and checks if it is tasked
        assert!(
            filter_payload
                .contains(&config.address)
                .expect("Should check filter"),
            "Node should be tasked"
        );
    }
}
