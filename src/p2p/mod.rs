use libp2p::StreamProtocol;

pub mod behaviour;
pub mod client;

pub const DRIA_PROTO_NAME: StreamProtocol = StreamProtocol::new("/dria/kad/1.0.0");
