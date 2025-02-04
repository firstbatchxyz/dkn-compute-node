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
                "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAmJjnAzHvjKMNLWN1ifPFsXkSXguzCkoxerZaF8gZYh5g6",
                "/ip4/18.234.39.91/tcp/4001/p2p/16Uiu2HAkwm9ZNrVp2Td85YDoyYHaAn3UuRB2bnExM6muRYUQj6gL",
                "/ip4/54.242.44.217/tcp/4001/p2p/16Uiu2HAmSGMTvisBqqwwMDdhywm9ZVeNerMiRH6Rz1UD4kSevgup",
                "/ip4/52.201.242.227/tcp/4001/p2p/16Uiu2HAm54MS9d4zrtXffLxgz6dpDKjaXPVHyheXg4iMSvCBBtHX",
              //  "/ip4/18.205.158.27/tcp/4001/p2p/16Uiu2HAmB2GFG8oYa7DuivXYEsMUPiiKQ6yq9haQHRHHjJrn7FHo",
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
               "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAmKT8m2mTh9ciHWdY112VH7WvkqEBpZAic7ETSvso9XAsf",
               "/ip4/18.232.93.227/tcp/4001/p2p/16Uiu2HAkzP6ZJuWJMRQdEzYapkvWGmycJkJTTvRhsePLox4DX4KU",
               "/ip4/54.157.219.194/tcp/4001/p2p/16Uiu2HAmHEo5r8eD1m77E99JfKVb4qQd2gZv4wd2DySpxqdjcWc8",
               "/ip4/54.88.171.104/tcp/4001/p2p/16Uiu2HAkvsLKDU5ufuU6DFsPfjkGSZh8qosLozWigR6WhJnCHgfU",
               // "/ip4/3.88.84.50/tcp/4001/p2p/16Uiu2HAmN35mw5MMf3SCUt3TpEU6WnBvjYJQ4ZZeKHRHEkSc4RPq",
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
              // "/ip4/3.238.84.83/tcp/4001/p2p/16Uiu2HAmEcBQRQy4CVCnQ144rnvagKa1fS5uAguwq9S2DRiRmAWE"
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
            // "16Uiu2HAmEcBQRQy4CVCnQ144rnvagKa1fS5uAguwq9S2DRiRmAWE",
            DriaNetworkType::Community => [].iter(),
            DriaNetworkType::Pro => [].iter(),
            DriaNetworkType::Test => [].iter(),
        }
        .map(|s: &&str| s.parse().expect("could not parse static rpc peer ids"))
        .collect()
    }
}
