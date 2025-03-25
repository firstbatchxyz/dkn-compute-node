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
    ///
    /// We could parse these from [`get_static_rpc_nodes`]
    /// but it's better to keep them separate in case we have different RPC peer ids.
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
