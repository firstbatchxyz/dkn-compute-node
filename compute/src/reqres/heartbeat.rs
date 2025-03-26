use std::time::Duration;

use dkn_p2p::libp2p::{request_response::OutboundRequestId, PeerId};
use dkn_workflows::{Model, ModelProvider};
use eyre::{eyre, Result};
use uuid::Uuid;

use super::IsResponder;
use serde::{Deserialize, Serialize};

use crate::DriaComputeNode;

pub struct HeartbeatRequester;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeartbeatRequest {
    /// A unique ID for the heartbeat request.
    pub(crate) heartbeat_id: Uuid,
    /// Deadline for the heartbeat request, in nanoseconds.
    pub(crate) deadline: chrono::DateTime<chrono::Utc>,
    /// Models available in the node.
    pub(crate) models: Vec<(ModelProvider, Model)>,
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
    /// Acknowledgement of the heartbeat.
    pub(crate) ack: bool,
}

impl IsResponder for HeartbeatRequester {
    type Request = HeartbeatRequest;
    type Response = HeartbeatResponse;
}

/// Any acknowledged heartbeat that is older than this duration is considered dead.
const HEARTBEAT_DEADLINE_SECS: Duration = Duration::from_secs(20);

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
            models: node.config.workflows.models.clone(),
            pending_tasks: node.get_pending_task_count(),
        };

        let request_id = node
            .p2p
            .request(
                peer_id,
                serde_json::to_vec(&heartbeat_request).expect("TODO: !!!"),
            )
            .await
            .expect("TODO: !!!");

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
            if !res.ack {
                Err(eyre!("Heartbeat was not acknowledged."))
            } else if chrono::Utc::now() > deadline {
                Err(eyre!("Acknowledged heartbeat was past the deadline."))
            } else {
                node.last_heartbeat_at = chrono::Utc::now();
                node.num_heartbeats += 1;

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
