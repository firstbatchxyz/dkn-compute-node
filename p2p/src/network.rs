use libp2p::{Multiaddr, PeerId};

/// Network type.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum DriaNetworkType {
    #[default]
    Community,
    Pro,
    Test,
}

impl From<&str> for DriaNetworkType {
    fn from(s: &str) -> Self {
        match s {
            "community" => DriaNetworkType::Community,
            "pro" => DriaNetworkType::Pro,
            "test" => DriaNetworkType::Test,
            _ => Default::default(),
        }
    }
}

impl std::fmt::Display for DriaNetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriaNetworkType::Community => write!(f, "community"),
            DriaNetworkType::Pro => write!(f, "pro"),
            DriaNetworkType::Test => write!(f, "test"),
        }
    }
}

impl DriaNetworkType {
    /// Returns the protocol name.
    pub fn protocol_name(&self) -> &str {
        match self {
            DriaNetworkType::Community => "dria",
            DriaNetworkType::Pro => "dria-sdk",
            DriaNetworkType::Test => "dria-test",
        }
    }

    /// Static bootstrap nodes for Kademlia.
    #[inline(always)]
    pub fn get_static_bootstrap_nodes(&self) -> Vec<Multiaddr> {
        match self {
             DriaNetworkType::Community => [
              "/ip4/18.205.158.27/tcp/4001/p2p/16Uiu2HAmB2GFG8oYa7DuivXYEsMUPiiKQ6yq9haQHRHHjJrn7FHo",
                //  "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4",
                //  "/ip4/18.234.39.91/tcp/4001/p2p/16Uiu2HAmJqegPzwuGKWzmb5m3RdSUJ7NhEGWB5jNCd3ca9zdQ9dU",
                //  "/ip4/54.242.44.217/tcp/4001/p2p/16Uiu2HAmR2sAoh9F8jT9AZup9y79Mi6NEFVUbwRvahqtWamfabkz",
                //  "/ip4/52.201.242.227/tcp/4001/p2p/16Uiu2HAmFEUCy1s1gjyHfc8jey4Wd9i5bSDnyFDbWTnbrF2J3KFb",
             ].iter(),
             DriaNetworkType::Pro => [].iter(),
             DriaNetworkType::Test => [].iter(),
         }
         .map(|s| s.parse().expect("could not parse static bootstrap address"))
         .collect()
    }

    /// Static relay nodes for the `P2pCircuit`.
    #[inline(always)]
    pub fn get_static_relay_nodes(&self) -> Vec<Multiaddr> {
        match self {
             DriaNetworkType::Community => [
              "/ip4/3.88.84.50/tcp/4001/p2p/16Uiu2HAmN35mw5MMf3SCUt3TpEU6WnBvjYJQ4ZZeKHRHEkSc4RPq",
                //  "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa",
                //  "/ip4/18.232.93.227/tcp/4001/p2p/16Uiu2HAmHeGKhWkXTweHJTA97qwP81ww1W2ntGaebeZ25ikDhd4z",
                //  "/ip4/54.157.219.194/tcp/4001/p2p/16Uiu2HAm7A5QVSy5FwrXAJdNNsdfNAcaYahEavyjnFouaEi22dcq",
                //  "/ip4/54.88.171.104/tcp/4001/p2p/16Uiu2HAm5WP1J6bZC3aHxd7XCUumMt9txAystmbZSaMS2omHepXa",
             ].iter(),
             DriaNetworkType::Pro => [].iter(),
             DriaNetworkType::Test => [].iter(),
         }
         .map(|s| s.parse().expect("could not parse static relay address"))
         .collect()
    }

    /// Static RPC Peer IDs.
    #[inline(always)]
    pub fn get_static_rpc_nodes(&self) -> Vec<Multiaddr> {
        match self {
            DriaNetworkType::Community => [
              "/ip4/3.238.173.11/tcp/4001/p2p/16Uiu2HAmEcBQRQy4CVCnQ144rnvagKa1fS5uAguwq9S2DRiRmAWE"
              ]
            .iter(),
            DriaNetworkType::Pro => [].iter(),
            DriaNetworkType::Test => [].iter(),
        }
        .map(|s: &&str| s.parse().expect("could not parse static rpc address"))
        .collect()
    }

    /// Static RPC Peer IDs.
    #[inline(always)]
    pub fn get_static_rpc_peer_ids(&self) -> Vec<PeerId> {
        match self {
            DriaNetworkType::Community => {
                ["16Uiu2HAmEcBQRQy4CVCnQ144rnvagKa1fS5uAguwq9S2DRiRmAWE"].iter()
            }
            DriaNetworkType::Pro => [].iter(),
            DriaNetworkType::Test => [].iter(),
        }
        .map(|s: &&str| s.parse().expect("could not parse static rpc peer ids"))
        .collect()
    }
}
