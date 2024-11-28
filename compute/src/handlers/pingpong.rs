use crate::{
    utils::{get_current_time_nanos, DKNMessage},
    DriaComputeNode,
};
use dkn_p2p::libp2p::gossipsub::MessageAcceptance;
use dkn_workflows::{Model, ModelProvider};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct PingpongHandler;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingpongPayload {
    /// UUID of the ping request, prevents replay attacks.
    uuid: String,
    /// Deadline for the ping request.
    deadline: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingpongResponse {
    pub(crate) uuid: String,
    pub(crate) models: Vec<(ModelProvider, Model)>,
    pub(crate) timestamp: u128,
}

impl PingpongHandler {
    pub(crate) const LISTEN_TOPIC: &'static str = "ping";
    pub(crate) const RESPONSE_TOPIC: &'static str = "pong";

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
        ping_message: &DKNMessage,
    ) -> Result<MessageAcceptance> {
        let pingpong = ping_message
            .parse_payload::<PingpongPayload>(true)
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

        // respond
        let response_body = PingpongResponse {
            uuid: pingpong.uuid.clone(),
            models: node.config.workflows.models.clone(),
            timestamp: get_current_time_nanos(),
        };

        // publish message
        let message = DKNMessage::new_signed(
            serde_json::json!(response_body).to_string(),
            Self::RESPONSE_TOPIC,
            &node.config.secret_key,
        );
        node.publish(message).await?;

        Ok(MessageAcceptance::Accept)
    }
}
