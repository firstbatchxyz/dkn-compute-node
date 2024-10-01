use async_trait::async_trait;
use eyre::Result;
use libp2p::gossipsub::MessageAcceptance;

mod pingpong;
pub use pingpong::PingpongHandler;

mod workflow;
pub use workflow::WorkflowHandler;

use crate::{utils::DKNMessage, DriaComputeNode};

#[async_trait]
pub trait ComputeHandler {
    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: DKNMessage,
        result_topic: &str,
    ) -> Result<MessageAcceptance>;
}
