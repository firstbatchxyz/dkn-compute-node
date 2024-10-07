use crate::{utils::DKNMessage, DriaComputeNode};
use async_trait::async_trait;
use dkn_p2p::libp2p::gossipsub::MessageAcceptance;
use eyre::Result;

mod pingpong;
pub use pingpong::PingpongHandler;

mod workflow;
pub use workflow::WorkflowHandler;

/// A DKN task is to be handled by the compute node, respecting this trait.
///
/// It is expected for the implemented handler to handle messages coming from `LISTEN_TOPIC`,
/// and then respond back to the `RESPONSE_TOPIC`.
#[async_trait]
pub trait ComputeHandler {
    /// Gossipsub topic name to listen for incoming messages from the network.
    const LISTEN_TOPIC: &'static str;
    /// Gossipsub topic name to respond with messages to the network.
    const RESPONSE_TOPIC: &'static str;

    /// A generic handler for DKN tasks.
    ///
    /// Returns a `MessageAcceptance` value that tells the P2P client to accept the incoming message.
    ///
    /// The handler has mutable reference to the compute node, and therefore can respond within the handler itself in any way it would like.
    async fn handle_compute(
        node: &mut DriaComputeNode,
        message: DKNMessage,
    ) -> Result<MessageAcceptance>;
}
