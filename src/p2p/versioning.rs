use libp2p::StreamProtocol;

/// Kademlia protocol prefix, as a macro so that it can be used in literal-expecting constants.
macro_rules! P2P_KADEMLIA_PREFIX {
    () => {
        "/dria/kad/"
    };
}
pub const P2P_KADEMLIA_PREFIX: &str = P2P_KADEMLIA_PREFIX!();

/// Identity protocol name prefix, as a macro so that it can be used in literal-expecting constants.
macro_rules! P2P_IDENTITY_PREFIX {
    () => {
        "dria/"
    };
}

/// Kademlia protocol version, in the form of `/dria/kad/<version>`, **notice the `/` at the start**.
///
/// It is important that this protocol matches EXACTLY among the nodes, otherwise there is a protocol-level logic
/// that will prevent peers from finding eachother within the DHT.
pub const P2P_KADEMLIA_PROTOCOL: StreamProtocol = StreamProtocol::new(concat!(
    P2P_KADEMLIA_PREFIX!(),
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
));

/// Protocol string, checked by Identify protocol handlers.
pub const P2P_PROTOCOL_STRING: &str = concat!(
    P2P_IDENTITY_PREFIX!(),
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
);
