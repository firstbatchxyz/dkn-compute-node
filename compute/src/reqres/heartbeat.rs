use dkn_p2p::libp2p::{request_response::OutboundRequestId, PeerId};
use dkn_utils::{
    payloads::{HeartbeatRequest, HeartbeatResponse},
    DriaMessage,
};
use eyre::{eyre, Result};
use std::time::Duration;
use uuid::Uuid;

use super::IsResponder;

use crate::DriaComputeNode;

pub struct HeartbeatRequester;

impl IsResponder for HeartbeatRequester {
    type Request = DriaMessage; // TODO: HeartbeatRequest;
    type Response = HeartbeatResponse;
}

/// Any acknowledged heartbeat that is older than this duration is considered dead.
const HEARTBEAT_DEADLINE_SECS: Duration = Duration::from_secs(20);

/// Topic for the [`DriaMessage`].
const HEARTBEAT_TOPIC: &str = "heartbeat";

impl HeartbeatRequester {
    pub(crate) async fn send_heartbeat(
        node: &mut DriaComputeNode,
        peer_id: PeerId,
    ) -> Result<OutboundRequestId> {
        let uuid = Uuid::new_v4();
        let deadline = chrono::Utc::now() + HEARTBEAT_DEADLINE_SECS;

        let heartbeat_request = HeartbeatRequest {
            heartbeat_id: uuid,
            deadline,
            models: node.config.workflows.get_model_names(),
            pending_tasks: node.get_pending_task_count(),
        };

        let heartbeat_message = node.new_message(
            serde_json::to_vec(&heartbeat_request).expect("should be serializable"),
            HEARTBEAT_TOPIC,
        );
        let request_id = node.p2p.request(peer_id, heartbeat_message).await?;

        // add it to local heartbeats set
        node.heartbeats.insert(uuid, deadline);

        Ok(request_id)
    }

    /// Handles the heartbeat request received from the network.
    pub(crate) async fn handle_ack(
        node: &mut DriaComputeNode,
        res: HeartbeatResponse,
    ) -> Result<()> {
        if let Some(deadline) = node.heartbeats.remove(&res.heartbeat_id) {
            if let Some(err) = res.error {
                Err(eyre!("Heartbeat was not acknowledged: {}", err))
            } else {
                // acknowledge heartbeat
                node.last_heartbeat_at = chrono::Utc::now();
                node.num_heartbeats += 1;

                // for diagnostics, we can check if the heartbeat was past its deadline as well
                if chrono::Utc::now() > deadline {
                    log::warn!("Acknowledged heartbeat was past its deadline.")
                }

                Ok(())
            }
        } else {
            Err(eyre!(
                "Received an unknown heartbeat response with UUID {}.",
                res.heartbeat_id
            ))
        }
    }
}
