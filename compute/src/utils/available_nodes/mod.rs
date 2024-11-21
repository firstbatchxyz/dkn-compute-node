use dkn_p2p::libp2p::{Multiaddr, PeerId};
use dkn_workflows::split_csv_line;
use eyre::Result;
use std::{env, fmt::Debug, str::FromStr};
use tokio::time::Instant;

mod statics;

use crate::DriaNetworkType;

/// Number of seconds between refreshing the available nodes.
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 30;

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
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub relay_nodes: Vec<Multiaddr>,
    pub rpc_nodes: Vec<PeerId>,
    pub rpc_addrs: Vec<Multiaddr>,
    pub last_refreshed: Instant,
    pub network_type: DriaNetworkType,
    pub refresh_interval_secs: u64,
}

impl AvailableNodes {
    /// Creates a new `AvailableNodes` struct for the given network type.
    pub fn new(network: DriaNetworkType) -> Self {
        Self {
            bootstrap_nodes: vec![],
            relay_nodes: vec![],
            rpc_nodes: vec![],
            rpc_addrs: vec![],
            last_refreshed: Instant::now(),
            network_type: network,
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
        }
    }

    /// Sets the refresh interval in seconds.
    pub fn with_refresh_interval(mut self, interval_secs: u64) -> Self {
        self.refresh_interval_secs = interval_secs;
        self
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

        // parse relay nodes
        let relay_nodes = split_csv_line(&env::var("DKN_RELAY_NODES").unwrap_or_default());
        if relay_nodes.is_empty() {
            log::debug!("No additional relay nodes provided.");
        } else {
            log::debug!("Using additional relay nodes: {:#?}", relay_nodes);
        }

        self.bootstrap_nodes = parse_vec(bootstrap_nodes);
        self.relay_nodes = parse_vec(relay_nodes);
    }

    /// Adds the static nodes to the struct, with respect to network type.
    pub fn populate_with_statics(&mut self) {
        self.bootstrap_nodes = self.network_type.get_static_bootstrap_nodes();
        self.relay_nodes = self.network_type.get_static_relay_nodes();
        self.rpc_nodes = self.network_type.get_static_rpc_peer_ids();
    }

    /// Joins the struct with another `AvailableNodes` struct.
    pub fn join(mut self, other: Self) -> Self {
        self.bootstrap_nodes.extend(other.bootstrap_nodes);
        self.relay_nodes.extend(other.relay_nodes);
        self.rpc_nodes.extend(other.rpc_nodes);
        self.rpc_addrs.extend(other.rpc_addrs);
        self
    }

    /// Removes duplicates within all fields.
    pub fn sort_dedup(mut self) -> Self {
        self.bootstrap_nodes.sort_unstable();
        self.bootstrap_nodes.dedup();

        self.relay_nodes.sort_unstable();
        self.relay_nodes.dedup();

        self.rpc_nodes.sort_unstable();
        self.rpc_nodes.dedup();

        self.rpc_addrs.sort_unstable();
        self.rpc_addrs.dedup();

        self
    }

    /// Returns whether enough time has passed since the last refresh.
    pub fn can_refresh(&self) -> bool {
        self.last_refreshed.elapsed().as_secs() > self.refresh_interval_secs
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
        let url = match self.network_type {
            DriaNetworkType::Community => "https://dkn.dria.co/available-nodes",
            DriaNetworkType::Pro => "https://dkn.dria.co/sdk/available-nodes",
        };
        let response = reqwest::get(url).await?;
        let response_body = response.json::<AvailableNodesApiResponse>().await?;

        self.bootstrap_nodes = parse_vec(response_body.bootstraps);
        self.relay_nodes = parse_vec(response_body.relays);
        self.rpc_nodes = parse_vec(response_body.rpcs);
        self.rpc_addrs = parse_vec(response_body.rpc_addrs);
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
