use crate::DriaNetworkType;
use dkn_p2p::libp2p::{Multiaddr, PeerId};

/// Static bootstrap nodes for the Kademlia DHT bootstrap step.
const STATIC_BOOTSTRAP_NODES: ([&str; 4], [&str; 0]) = (
    // community
    [
        "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4",
        "/ip4/18.234.39.91/tcp/4001/p2p/16Uiu2HAmJqegPzwuGKWzmb5m3RdSUJ7NhEGWB5jNCd3ca9zdQ9dU",
        "/ip4/54.242.44.217/tcp/4001/p2p/16Uiu2HAmR2sAoh9F8jT9AZup9y79Mi6NEFVUbwRvahqtWamfabkz",
        "/ip4/52.201.242.227/tcp/4001/p2p/16Uiu2HAmFEUCy1s1gjyHfc8jey4Wd9i5bSDnyFDbWTnbrF2J3KFb",
    ],
    // pro
    [],
);

/// Static relay nodes for the `P2pCircuit`.
const STATIC_RELAY_NODES: ([&str; 4], [&str; 0]) = (
    // community
    [
        "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa",
        "/ip4/18.232.93.227/tcp/4001/p2p/16Uiu2HAmHeGKhWkXTweHJTA97qwP81ww1W2ntGaebeZ25ikDhd4z",
        "/ip4/54.157.219.194/tcp/4001/p2p/16Uiu2HAm7A5QVSy5FwrXAJdNNsdfNAcaYahEavyjnFouaEi22dcq",
        "/ip4/54.88.171.104/tcp/4001/p2p/16Uiu2HAm5WP1J6bZC3aHxd7XCUumMt9txAystmbZSaMS2omHepXa",
    ],
    // pro
    [],
);

/// Static RPC Peer IDs for the Admin RPC.
const STATIC_RPC_PEER_IDS: ([&str; 0], [&str; 0]) = (
    // community
    [],
    // pro
    [],
);

impl DriaNetworkType {
    // TODO: kind of smelly code here
    pub fn get_static_bootstrap_nodes(&self) -> Vec<Multiaddr> {
        match self {
            DriaNetworkType::Community => STATIC_BOOTSTRAP_NODES.0.iter(),
            DriaNetworkType::Pro => STATIC_BOOTSTRAP_NODES.1.iter(),
        }
        .filter_map(|s| s.parse().ok())
        .collect()
    }

    pub fn get_static_relay_nodes(&self) -> Vec<Multiaddr> {
        match self {
            DriaNetworkType::Community => STATIC_RELAY_NODES.0.iter(),
            DriaNetworkType::Pro => STATIC_RELAY_NODES.1.iter(),
        }
        .filter_map(|s| s.parse().ok())
        .collect()
    }

    pub fn get_static_rpc_peer_ids(&self) -> Vec<PeerId> {
        match self {
            DriaNetworkType::Community => STATIC_RPC_PEER_IDS.0.iter(),
            DriaNetworkType::Pro => STATIC_RPC_PEER_IDS.1.iter(),
        }
        .filter_map(|s| s.parse().ok())
        .collect()
    }
}

// help me
