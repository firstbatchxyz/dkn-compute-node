use libp2p::StreamProtocol;

/// Kademlia protocol version, in the form of `/dria/kad/<version>`.
/// Notice the `/` at the start.
pub(crate) const P2P_KADEMLIA_PROTOCOL: StreamProtocol =
    StreamProtocol::new(concat!("/dria/kad/", env!("CARGO_PKG_VERSION")));

/// Protocol string, checked by Identify protocol
pub(crate) const P2P_PROTOCOL_STRING: &str = concat!("dria/", env!("CARGO_PKG_VERSION"));

mod behaviour;
pub use behaviour::{DriaBehaviour, DriaBehaviourEvent};

mod client;
pub use client::P2PClient;

mod message;

pub use message::P2PMessage;

mod available_nodes;
pub use available_nodes::AvailableNodes;

mod data_transform;
