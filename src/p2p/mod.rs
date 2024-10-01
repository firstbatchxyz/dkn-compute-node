use libp2p::StreamProtocol;

/// Kademlia protocol prefix, as a macro so that it can be used in const macros.
macro_rules! P2P_KADEMLIA_PREFIX {
    () => {
        "/dria/kad/"
    };
}

/// Kademlia protocol prefix, as a macro so that it can be used in const macros.
macro_rules! P2P_IDENTITY_PREFIX {
    () => {
        "dria/"
    };
}

/// Kademlia protocol version, in the form of `/dria/kad/<version>`.
/// Notice the `/` at the start.
pub(crate) const P2P_KADEMLIA_PROTOCOL: StreamProtocol = StreamProtocol::new(concat!(
    P2P_KADEMLIA_PREFIX!(),
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
));

/// Protocol string, checked by Identify protocol
pub(crate) const P2P_PROTOCOL_STRING: &str = concat!(
    P2P_IDENTITY_PREFIX!(),
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
);

mod behaviour;
pub use behaviour::{DriaBehaviour, DriaBehaviourEvent};

mod client;
pub use client::P2PClient;

mod data_transform;
