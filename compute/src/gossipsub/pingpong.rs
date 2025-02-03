use dkn_p2p::libp2p::gossipsub::MessageAcceptance;
use dkn_utils::get_current_time_nanos;
use dkn_workflows::{Model, ModelProvider};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::{utils::DriaMessage, DriaComputeNode};

pub struct PingpongHandler;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PingpongPayload {
    /// UUID of the ping request, prevents replay attacks.
    uuid: String,
    /// Deadline for the ping request.
    deadline: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PingpongResponse {
    /// UUID as given in the ping payload.
    pub(crate) uuid: String,
    /// Models available in the node.
    pub(crate) models: Vec<(ModelProvider, Model)>,
    /// Number of tasks in the channel currently, `single` and `batch`.
    pub(crate) pending_tasks: [usize; 2],
}

impl PingpongHandler {
    pub const LISTEN_TOPIC: &'static str = "ping";
    pub const RESPONSE_TOPIC: &'static str = "pong";

    /// Handles the ping message and responds with a pong message.
    ///
    /// 1. Parses the payload of the incoming message into a `PingpongPayload`.
    /// 2. Checks if the current time is past the deadline specified in the ping request.
    /// 3. If the current time is past the deadline, logs a debug message and ignores the ping request.
    /// 4. If the current time is within the deadline, constructs a `PingpongResponse` with the UUID from the ping request, the models from the node's configuration, and the current timestamp.
    /// 5. Creates a new signed `DKNMessage` with the response body and the `RESPONSE_TOPIC`.
    /// 6. Publishes the response message.
    /// 7. Returns `MessageAcceptance::Accept` so that ping is propagated to others as well.
    pub(crate) async fn handle_ping(
        node: &mut DriaComputeNode,
        ping_message: &DriaMessage,
    ) -> Result<MessageAcceptance> {
        let pingpong = ping_message
            .parse_payload::<PingpongPayload>()
            .wrap_err("could not parse ping request")?;

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

        log::info!("Received a ping for: {}", pingpong.uuid);

        // record ping moment
        node.last_pinged_at = Instant::now();

        // respond
        let response_body = PingpongResponse {
            uuid: pingpong.uuid.clone(),
            models: node.config.workflows.models.clone(),
            pending_tasks: node.get_pending_task_count(),
        };

        // publish message
        let message = node.new_message(
            serde_json::json!(response_body).to_string(),
            Self::RESPONSE_TOPIC,
        );
        node.publish(message).await?;

        Ok(MessageAcceptance::Accept)
    }
}
