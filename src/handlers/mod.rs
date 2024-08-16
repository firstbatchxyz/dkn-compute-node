use async_trait::async_trait;
use libp2p::gossipsub::MessageAcceptance;

mod pingpong;
pub use pingpong::PingpongHandler;

mod workflow;
pub use workflow::WorkflowHandler;

use crate::{errors::NodeResult, p2p::P2PMessage, DriaComputeNode};

#[async_trait]
pub trait ComputeHandler {
    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: P2PMessage,
        result_topic: &str,
    ) -> NodeResult<MessageAcceptance>;
}
