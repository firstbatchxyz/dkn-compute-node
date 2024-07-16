use libp2p::StreamProtocol;

mod behaviour;
mod client;
mod message;

pub const DRIA_PROTO_NAME: StreamProtocol = StreamProtocol::new("/dria/kad/1.0.0");

pub use client::P2PClient;
pub use message::P2PMessage;
