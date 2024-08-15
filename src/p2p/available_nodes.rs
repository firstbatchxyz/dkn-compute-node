use libp2p::{Multiaddr, PeerId};
use std::{fmt::Debug, str::FromStr};

use crate::utils::split_comma_separated;

/// Static bootstrap nodes for the Kademlia DHT bootstrap step.
const STATIC_BOOTSTRAP_NODES: [&str; 4] = [
    "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4",
    "/ip4/18.234.39.91/tcp/4001/p2p/16Uiu2HAmJqegPzwuGKWzmb5m3RdSUJ7NhEGWB5jNCd3ca9zdQ9dU",
    "/ip4/54.242.44.217/tcp/4001/p2p/16Uiu2HAmR2sAoh9F8jT9AZup9y79Mi6NEFVUbwRvahqtWamfabkz",
    "/ip4/52.201.242.227/tcp/4001/p2p/16Uiu2HAmFEUCy1s1gjyHfc8jey4Wd9i5bSDnyFDbWTnbrF2J3KFb",
];

/// Static relay nodes for the `P2pCircuit`.
const STATIC_RELAY_NODES: [&str; 4] = [
    "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa",
    "/ip4/18.232.93.227/tcp/4001/p2p/16Uiu2HAmHeGKhWkXTweHJTA97qwP81ww1W2ntGaebeZ25ikDhd4z",
    "/ip4/54.157.219.194/tcp/4001/p2p/16Uiu2HAm7A5QVSy5FwrXAJdNNsdfNAcaYahEavyjnFouaEi22dcq",
    "/ip4/54.88.171.104/tcp/4001/p2p/16Uiu2HAm5WP1J6bZC3aHxd7XCUumMt9txAystmbZSaMS2omHepXa",
];

/// Static RPC Peer IDs for the Admin RPC.
const STATIC_RPC_PEER_IDS: [&str; 0] = [];

/// API URL for refreshing the Admin RPC PeerIDs from Dria server.
const RPC_PEER_ID_REFRESH_API_URL: &str = "https://dkn.dria.co/available-nodes";

#[derive(serde::Deserialize, Debug)]
pub struct AvailableNodesApiResponse {
    pub bootstraps: Vec<String>,
    pub relays: Vec<String>,
    pub rpcs: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct AvailableNodes {
    pub bootstrap_nodes: Vec<Multiaddr>,
    pub relay_nodes: Vec<Multiaddr>,
    pub rpc_nodes: Vec<PeerId>,
}

impl AvailableNodes {
    /// Parses static bootstrap & relay nodes from environment variables.
    ///
    /// The environment variables are:
    /// - `DRIA_BOOTSTRAP_NODES`: comma-separated list of bootstrap nodes
    /// - `DRIA_RELAY_NODES`: comma-separated list of relay nodes
    pub fn new_from_env() -> Self {
        // parse bootstrap nodes
        let bootstrap_nodes = split_comma_separated(std::env::var("DKN_BOOTSTRAP_NODES").ok());
        if bootstrap_nodes.is_empty() {
            log::debug!("No additional bootstrap nodes provided.");
        } else {
            log::debug!("Using additional bootstrap nodes: {:#?}", bootstrap_nodes);
        }

        // parse relay nodes
        let relay_nodes = split_comma_separated(std::env::var("DKN_RELAY_NODES").ok());
        if relay_nodes.is_empty() {
            log::debug!("No additional relay nodes provided.");
        } else {
            log::debug!("Using additional relay nodes: {:#?}", relay_nodes);
        }

        Self {
            bootstrap_nodes: parse_vec(bootstrap_nodes),
            relay_nodes: parse_vec(relay_nodes),
            rpc_nodes: vec![],
        }
    }

    pub fn new_from_statics() -> Self {
        Self {
            bootstrap_nodes: parse_vec(STATIC_BOOTSTRAP_NODES.to_vec()),
            relay_nodes: parse_vec(STATIC_RELAY_NODES.to_vec()),
            rpc_nodes: parse_vec(STATIC_RPC_PEER_IDS.to_vec()),
        }
    }

    /// Joins the available nodes from another `AvailableNodes` struct.
    pub fn join(mut self, other: Self) -> Self {
        self.bootstrap_nodes.extend(other.bootstrap_nodes);
        self.relay_nodes.extend(other.relay_nodes);
        self.rpc_nodes.extend(other.rpc_nodes);

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

        self
    }

    /// Refreshes the available nodes for Bootstrap, Relay and RPC nodes.
    ///
    /// Returns immediately if its too early to do that.
    pub async fn get_available_nodes() -> Result<Self, reqwest::Error> {
        let response = reqwest::get(RPC_PEER_ID_REFRESH_API_URL).await?;
        let response_body = response.json::<AvailableNodesApiResponse>().await?;

        Ok(Self {
            bootstrap_nodes: parse_vec(response_body.bootstraps),
            relay_nodes: parse_vec(response_body.relays),
            rpc_nodes: parse_vec(response_body.rpcs),
        })
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
        std::env::set_var("RUST_LOG", "info");
        let _ = env_logger::try_init();

        let available_nodes = AvailableNodes::get_available_nodes().await.unwrap();
        println!("{:#?}", available_nodes);
    }
}
