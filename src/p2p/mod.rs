use libp2p::StreamProtocol;

pub(crate) const P2P_KADEMLIA_PROTOCOL: StreamProtocol = StreamProtocol::new("/dria/kad/1.0.0");
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
