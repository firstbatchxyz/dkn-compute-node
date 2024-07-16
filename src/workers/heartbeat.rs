use crate::{errors::NodeResult, node::DriaComputeNode, p2p::P2PMessage};
use ollama_workflows::{Model, ModelProvider};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatPayload {
    uuid: String,
    deadline: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HeartbeatResponse {
    pub(crate) uuid: String,
    pub(crate) models: Vec<(ModelProvider, Model)>,
}

pub trait HandlesHeartbeat {
    /// # Heartbeat
    ///
    /// A heartbeat is a message sent by a node to indicate that it is alive. Dria nodes request
    /// a heartbeat with a unique identifier, and the requester node will sign the identifier and send the signature back to a topic
    /// identified with the `uuid`.
    fn handle_heartbeat(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()>;
}

impl HandlesHeartbeat for DriaComputeNode {
    fn handle_heartbeat(&mut self, message: P2PMessage, result_topic: &str) -> NodeResult<()> {
        let request_body = message.parse_payload::<HeartbeatPayload>(true)?;
        let response_body = HeartbeatResponse {
            uuid: request_body.uuid.clone(),
            models: self.config.models.clone(),
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

// #[cfg(test)]
// mod tests {
//     use crate::{
//         config::constants::DEFAULT_DKN_ADMIN_PUBLIC_KEY,
//         node::DriaComputeNode,
//         utils::{
//             crypto::{sha256hash, to_address},
//             filter::FilterPayload,
//         },
//         waku::message::P2PMessage,
//     };
//     use fastbloom_rs::{FilterBuilder, Membership};
//     use libsecp256k1::{recover, Message, PublicKey};

//     use super::HeartbeatPayload;

//     #[test]
//     fn test_heartbeat_payload() {
//         let pk = PublicKey::parse_compressed(DEFAULT_DKN_ADMIN_PUBLIC_KEY)
//             .expect("Should parse public key");
//         let message = P2PMessage {
//             payload: "Y2RmODcyNDlhY2U3YzQ2MDIzYzNkMzBhOTc4ZWY3NjViMWVhZDlmNWJhMDUyY2MxMmY0NzIzMjQyYjc0YmYyODFjMDA1MTdmMGYzM2VkNTgzMzk1YWUzMTY1ODQ3NWQyNDRlODAxYzAxZDE5MjYwMDM1MTRkNzEwMThmYTJkNjEwMXsidXVpZCI6ICI4MWE2M2EzNC05NmM2LTRlNWEtOTliNS02YjI3NGQ5ZGUxNzUiLCAiZGVhZGxpbmUiOiAxNzE0MTI4NzkyfQ==".to_string(),
//             content_topic: "/dria/0/heartbeat/proto".to_string(),
//             version: 0,
//             timestamp: 1714129073557846272,
//             ephemeral: true
//         };

//         assert!(message.is_signed(&pk).expect("Should check signature"));

//         let obj = message
//             .parse_payload::<HeartbeatPayload>(true)
//             .expect("Should parse payload");
//         assert_eq!(obj.uuid, "81a63a34-96c6-4e5a-99b5-6b274d9de175");
//         assert_eq!(obj.deadline, 1714128792);
//     }

//     /// This test demonstrates the process of heartbeat & task assignment.
//     ///
//     /// A heart-beat message is sent over the network by Admin Node, and compute node responds with a signature.
//     #[test]
//     fn test_heartbeat_and_task_assignment() {
//         let node = DriaComputeNode::default();

//         // a heartbeat message is signed and sent to Admin Node (via Waku network)
//         let heartbeat_message = Message::parse(&sha256hash(b"sign-me"));
//         let (heartbeat_signature, heartbeat_recid) = node.sign(&heartbeat_message);

//         // admin recovers the address from the signature
//         let recovered_public_key =
//             recover(&heartbeat_message, &heartbeat_signature, &heartbeat_recid)
//                 .expect("Could not recover");
//         assert_eq!(
//             node.config.public_key, recovered_public_key,
//             "Public key mismatch"
//         );
//         let address = to_address(&recovered_public_key);
//         assert_eq!(address, node.config.address, "Address mismatch");

//         // admin node assigns the task to the compute node via Bloom Filter
//         let mut bloom = FilterBuilder::new(100, 0.01).build_bloom_filter();
//         bloom.add(&address);
//         let filter_payload = FilterPayload::from(bloom);

//         // compute node receives the filter and checks if it is tasked
//         assert!(
//             node.is_tasked(&filter_payload)
//                 .expect("Should check filter"),
//             "Node should be tasked"
//         );
//     }
// }
