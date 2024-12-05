use crate::DriaNetworkType;
use dkn_utils::{parse_vec, split_csv_line};
use libp2p::{Multiaddr, PeerId};
use std::{collections::HashSet, env, fmt::Debug};

/// Dria-owned nodes within the hybrid P2P network.
///
/// - Bootstrap: used for Kademlia DHT bootstrap.
/// - Relay: used for DCutR relay protocol.
/// - RPC: used for RPC nodes for task & ping messages.
#[derive(Debug, Clone)]
pub struct DriaNodes {
    pub bootstrap_nodes: HashSet<Multiaddr>,
    pub relay_nodes: HashSet<Multiaddr>,
    pub rpc_nodes: HashSet<Multiaddr>,
    pub rpc_peerids: HashSet<PeerId>,
    pub network: DriaNetworkType,
}

impl DriaNodes {
    /// Creates a new `AvailableNodes` struct for the given network type.
    pub fn new(network: DriaNetworkType) -> Self {
        Self {
            bootstrap_nodes: HashSet::new(),
            relay_nodes: HashSet::new(),
            rpc_nodes: HashSet::new(),
            rpc_peerids: HashSet::new(),
            network,
        }
    }

    pub fn with_relay_nodes(mut self, addresses: impl IntoIterator<Item = Multiaddr>) -> Self {
        self.relay_nodes.extend(addresses);
        self
    }

    pub fn with_bootstrap_nodes(mut self, addresses: impl IntoIterator<Item = Multiaddr>) -> Self {
        self.bootstrap_nodes.extend(addresses);
        self
    }

    pub fn with_rpc_nodes(mut self, addresses: impl IntoIterator<Item = Multiaddr>) -> Self {
        self.rpc_nodes.extend(addresses);
        self
    }

    pub fn with_rpc_peer_ids(mut self, addresses: impl IntoIterator<Item = PeerId>) -> Self {
        self.rpc_peerids.extend(addresses);
        self
    }

    /// Parses static bootstrap & relay nodes from environment variables.
    ///
    /// The environment variables are:
    /// - `DRIA_BOOTSTRAP_NODES`: comma-separated list of bootstrap nodes
    /// - `DRIA_RELAY_NODES`: comma-separated list of relay nodes
    pub fn with_envs(mut self) -> Self {
        // parse bootstrap nodes
        let bootstrap_nodes = split_csv_line(&env::var("DKN_BOOTSTRAP_NODES").unwrap_or_default());
        if bootstrap_nodes.is_empty() {
            log::debug!("No additional bootstrap nodes provided.");
        } else {
            log::debug!("Using additional bootstrap nodes: {:#?}", bootstrap_nodes);
        }
        self.bootstrap_nodes
            .extend(parse_vec(bootstrap_nodes).expect("could not parse bootstrap nodes"));

        // parse relay nodes
        let relay_nodes = split_csv_line(&env::var("DKN_RELAY_NODES").unwrap_or_default());
        if relay_nodes.is_empty() {
            log::debug!("No additional relay nodes provided.");
        } else {
            log::debug!("Using additional relay nodes: {:#?}", relay_nodes);
        }
        self.relay_nodes
            .extend(parse_vec(relay_nodes).expect("could not parse relay nodes"));

        self
    }

    /// Adds the static nodes to the struct, with respect to network type.
    pub fn with_statics(mut self) -> Self {
        self.bootstrap_nodes
            .extend(self.network.get_static_bootstrap_nodes());
        self.relay_nodes
            .extend(self.network.get_static_relay_nodes());
        self.rpc_peerids
            .extend(self.network.get_static_rpc_peer_ids());

        self
    }
}
