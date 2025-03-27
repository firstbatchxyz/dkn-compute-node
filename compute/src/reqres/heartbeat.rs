use dkn_p2p::libp2p::{request_response::OutboundRequestId, PeerId};
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use super::IsResponder;

use crate::{utils::DriaMessage, DriaComputeNode};

pub struct HeartbeatRequester;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartbeatRequest {
    /// A unique ID for the heartbeat request.
    pub(crate) heartbeat_id: Uuid,
    /// Deadline for the heartbeat request, in nanoseconds.
    pub(crate) deadline: chrono::DateTime<chrono::Utc>,
    /// Model names available in the node.
    pub(crate) models: Vec<String>,
    /// Number of tasks in the channel currently, `single` and `batch`.
    pub(crate) pending_tasks: [usize; 2],
}

/// The response is an object with UUID along with an ACK (acknowledgement).
///
/// If for any reason the `ack` is `false`, the request is considered failed.
/// This may be when `deadline` is past the current time, or if the node is deeming itself unhealthy.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartbeatResponse {
    /// UUID as given in the request.
    pub(crate) heartbeat_id: Uuid,
    /// An associated error with the response:
    /// - `None` means that the heartbeat was acknowledged.
    /// - `Some` means that the heartbeat was not acknowledged for the given reason.
    pub(crate) error: Option<String>,
}

impl IsResponder for HeartbeatRequester {
    type Request = DriaMessage; // TODO: HeartbeatRequest;
    type Response = DriaMessage; // TODO: HeartbeatResponse;
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
            models: node
                .config
                .workflows
                .models
                .iter()
                .map(|m| m.1.to_string())
                .collect(),
            pending_tasks: node.get_pending_task_count(),
        };

        let heartbeat_message = node.new_message(
            serde_json::to_vec(&heartbeat_request).expect("should be serializable"),
            HEARTBEAT_TOPIC,
        );
        let request_id = node
            .p2p
            .request(peer_id, heartbeat_message.to_bytes()?)
            .await?;

        // add it to local heartbeats set
        node.heartbeats.insert(uuid, deadline);

        Ok(request_id)
    }

    /// Handles the heartbeat request received from the network.
    pub(crate) async fn handle_ack(
        node: &mut DriaComputeNode,
        ack_message: DriaMessage,
    ) -> Result<()> {
        let res = ack_message.parse_payload::<HeartbeatResponse>()?;

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
