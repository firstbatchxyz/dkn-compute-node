use dkn_p2p::libp2p::{Multiaddr, PeerId};
use dkn_workflows::split_csv_line;
use eyre::Result;
use std::{collections::HashSet, env, fmt::Debug, str::FromStr};
use tokio::time::Instant;

mod statics;

use crate::DriaNetworkType;

impl DriaNetworkType {
    /// Returns the URL for fetching available nodes w.r.t network type.
    pub fn get_available_nodes_url(&self) -> &str {
        match self {
            DriaNetworkType::Community => "https://dkn.dria.co/available-nodes",
            DriaNetworkType::Pro => "https://dkn.dria.co/sdk/available-nodes",
        }
    }
}
/// Available nodes within the hybrid P2P network.
///
/// - Bootstrap: used for Kademlia DHT bootstrap.
/// - Relay: used for DCutR relay protocol.
/// - RPC: used for RPC nodes for task & ping messages.
///
/// Note that while bootstrap & relay nodes are `Multiaddr`, RPC nodes are `PeerId` because we communicate
/// with them via GossipSub only.
#[derive(Debug, Clone)]
pub struct AvailableNodes {
    pub bootstrap_nodes: HashSet<Multiaddr>,
    pub relay_nodes: HashSet<Multiaddr>,
    pub rpc_nodes: HashSet<PeerId>,
    pub rpc_addrs: HashSet<Multiaddr>,
    pub last_refreshed: Instant,
    pub network_type: DriaNetworkType,
}

impl AvailableNodes {
    /// Creates a new `AvailableNodes` struct for the given network type.
    pub fn new(network: DriaNetworkType) -> Self {
        Self {
            bootstrap_nodes: HashSet::new(),
            relay_nodes: HashSet::new(),
            rpc_nodes: HashSet::new(),
            rpc_addrs: HashSet::new(),
            last_refreshed: Instant::now(),
            network_type: network,
        }
    }

    /// Parses static bootstrap & relay nodes from environment variables.
    ///
    /// The environment variables are:
    /// - `DRIA_BOOTSTRAP_NODES`: comma-separated list of bootstrap nodes
    /// - `DRIA_RELAY_NODES`: comma-separated list of relay nodes
    pub fn populate_with_env(&mut self) {
        // parse bootstrap nodes
        let bootstrap_nodes = split_csv_line(&env::var("DKN_BOOTSTRAP_NODES").unwrap_or_default());
        if bootstrap_nodes.is_empty() {
            log::debug!("No additional bootstrap nodes provided.");
        } else {
            log::debug!("Using additional bootstrap nodes: {:#?}", bootstrap_nodes);
        }
        self.bootstrap_nodes.extend(parse_vec(bootstrap_nodes));

        // parse relay nodes
        let relay_nodes = split_csv_line(&env::var("DKN_RELAY_NODES").unwrap_or_default());
        if relay_nodes.is_empty() {
            log::debug!("No additional relay nodes provided.");
        } else {
            log::debug!("Using additional relay nodes: {:#?}", relay_nodes);
        }
        self.relay_nodes.extend(parse_vec(relay_nodes));
    }

    /// Adds the static nodes to the struct, with respect to network type.
    pub fn populate_with_statics(&mut self) {
        self.bootstrap_nodes
            .extend(self.network_type.get_static_bootstrap_nodes());
        self.relay_nodes
            .extend(self.network_type.get_static_relay_nodes());
        self.rpc_nodes
            .extend(self.network_type.get_static_rpc_peer_ids());
    }

    /// Refresh available nodes using the API.
    pub async fn populate_with_api(&mut self) -> Result<()> {
        #[derive(serde::Deserialize, Default, Debug)]
        struct AvailableNodesApiResponse {
            pub bootstraps: Vec<String>,
            pub relays: Vec<String>,
            pub rpcs: Vec<String>,
            #[serde(rename = "rpcAddrs")]
            pub rpc_addrs: Vec<String>,
        }

        // make the request w.r.t network type
        let response = reqwest::get(self.network_type.get_available_nodes_url()).await?;
        let response_body = response.json::<AvailableNodesApiResponse>().await?;
        self.bootstrap_nodes
            .extend(parse_vec(response_body.bootstraps));
        self.relay_nodes.extend(parse_vec(response_body.relays));
        self.rpc_addrs.extend(parse_vec(response_body.rpc_addrs));
        self.rpc_nodes
            .extend(parse_vec::<PeerId>(response_body.rpcs));
        self.last_refreshed = Instant::now();

        Ok(())
    }
}

/// Like `parse` of `str` but for vectors.
fn parse_vec<T>(input: Vec<impl AsRef<str> + Debug>) -> Vec<T>
where
    T: FromStr,
{
    let parsed = input
        .iter()
        .filter_map(|s| s.as_ref().parse::<T>().ok())
        .collect::<Vec<_>>();

    if parsed.len() != input.len() {
        log::warn!("Some inputs could not be parsed: {:?}", input);
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_get_available_nodes() {
        let mut available_nodes = AvailableNodes::new(DriaNetworkType::Community);
        available_nodes.populate_with_api().await.unwrap();
        println!("Community: {:#?}", available_nodes);

        let mut available_nodes = AvailableNodes::new(DriaNetworkType::Pro);
        available_nodes.populate_with_api().await.unwrap();
        println!("Pro: {:#?}", available_nodes);
    }
}
