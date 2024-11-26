use super::ComputeHandler;
use crate::{
    utils::{get_current_time_nanos, DKNMessage},
    DriaComputeNode,
};
use async_trait::async_trait;
use dkn_p2p::libp2p::gossipsub::MessageAcceptance;
use dkn_workflows::{Model, ModelProvider};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};

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
        let pingpong = message
            .parse_payload::<PingpongPayload>(true)
            .wrap_err("Could not parse ping request")?;

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
