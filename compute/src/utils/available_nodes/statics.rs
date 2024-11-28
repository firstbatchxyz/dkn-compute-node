use crate::DriaNetworkType;
use dkn_p2p::libp2p::{Multiaddr, PeerId};

impl DriaNetworkType {
    /// Static bootstrap nodes for Kademlia.
    #[inline(always)]
    pub fn get_static_bootstrap_nodes(&self) -> Vec<Multiaddr> {
        match self {
            DriaNetworkType::Community => [
                "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4",
                "/ip4/18.234.39.91/tcp/4001/p2p/16Uiu2HAmJqegPzwuGKWzmb5m3RdSUJ7NhEGWB5jNCd3ca9zdQ9dU",
                "/ip4/54.242.44.217/tcp/4001/p2p/16Uiu2HAmR2sAoh9F8jT9AZup9y79Mi6NEFVUbwRvahqtWamfabkz",
                "/ip4/52.201.242.227/tcp/4001/p2p/16Uiu2HAmFEUCy1s1gjyHfc8jey4Wd9i5bSDnyFDbWTnbrF2J3KFb",
            ].iter(),
            DriaNetworkType::Pro => [].iter(),
        }
        .map(|s| s.parse().expect("could not parse static bootstrap address"))
        .collect()
    }

    /// Static relay nodes for the `P2pCircuit`.
    #[inline(always)]
    pub fn get_static_relay_nodes(&self) -> Vec<Multiaddr> {
        match self {
            DriaNetworkType::Community => [
                "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa",
                "/ip4/18.232.93.227/tcp/4001/p2p/16Uiu2HAmHeGKhWkXTweHJTA97qwP81ww1W2ntGaebeZ25ikDhd4z",
                "/ip4/54.157.219.194/tcp/4001/p2p/16Uiu2HAm7A5QVSy5FwrXAJdNNsdfNAcaYahEavyjnFouaEi22dcq",
                "/ip4/54.88.171.104/tcp/4001/p2p/16Uiu2HAm5WP1J6bZC3aHxd7XCUumMt9txAystmbZSaMS2omHepXa",
            ].iter(),
            DriaNetworkType::Pro => [].iter(),
        }
        .map(|s| s.parse().expect("could not parse static relay address"))
        .collect()
    }

    /// Static RPC Peer IDs for the Admin RPC.
    #[inline(always)]
    pub fn get_static_rpc_peer_ids(&self) -> Vec<PeerId> {
        // match self {
        //     DriaNetworkType::Community => [].iter(),
        //     DriaNetworkType::Pro => [].iter(),
        // }
        // .filter_map(|s| s.parse().ok())
        // .collect()
        vec![]
    }
}

// help me
