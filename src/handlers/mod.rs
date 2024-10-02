use async_trait::async_trait;
use eyre::Result;
use libp2p::gossipsub::MessageAcceptance;

mod topics;
pub use topics::*;

mod pingpong;
pub use pingpong::PingpongHandler;

mod workflow;
pub use workflow::WorkflowHandler;

use crate::{utils::DKNMessage, DriaComputeNode};

/// A DKN task is to be handled by the compute node, respecting this trait.
#[async_trait]
pub trait ComputeHandler {
    /// A generic handler for DKN tasks.
    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: DKNMessage,
        result_topic: &str,
    ) -> Result<MessageAcceptance>;
}
