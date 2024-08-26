use libp2p::StreamProtocol;
pub const DRIA_PROTO_NAME: StreamProtocol = StreamProtocol::new("/dria/kad/1.0.0");

mod behaviour;
pub use behaviour::{DriaBehaviour, DriaBehaviourEvent};

mod client;
pub use client::P2PClient;

mod message;
pub use message::P2PMessage;

mod available_nodes;
pub use available_nodes::AvailableNodes;

mod data_transform;
