use libp2p::{Multiaddr, PeerId};
use std::{collections::HashSet, fmt::Debug};

use crate::DriaNetworkType;

/// Dria-owned nodes within the hybrid P2P network.
///
/// - RPC: used for RPC nodes for task & ping messages.
#[derive(Debug, Clone)]
pub struct DriaNodes {
    pub rpc_nodes: HashSet<Multiaddr>,
    pub rpc_peerids: HashSet<PeerId>,
    pub network: DriaNetworkType,
}

impl DriaNodes {
    /// Creates a new `AvailableNodes` struct for the given network type.
    pub fn new(network: DriaNetworkType) -> Self {
        Self {
            rpc_nodes: HashSet::new(),
            rpc_peerids: HashSet::new(),
            network,
        }
    }
    pub fn with_rpc_nodes(mut self, addresses: impl IntoIterator<Item = Multiaddr>) -> Self {
        self.rpc_nodes.extend(addresses);
        self
    }

    pub fn with_rpc_peer_ids(mut self, addresses: impl IntoIterator<Item = PeerId>) -> Self {
        self.rpc_peerids.extend(addresses);
        self
    }

    /// Adds the static nodes to the struct, with respect to network type.
    pub fn with_statics(mut self) -> Self {
        self.rpc_nodes.extend(self.network.get_static_rpc_nodes());
        self.rpc_peerids
            .extend(self.network.get_static_rpc_peer_ids());

        self
    }
}
