use dkn_p2p::libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use dkn_utils::{DriaNetwork, SemanticVersion};
use eyre::{Context, OptionExt, Result};
use std::fmt::Debug;

/// The connected RPC node, as per the Star network topology.
#[derive(Debug, Clone)]
pub struct DriaRPC {
    pub addr: Multiaddr,
    pub peer_id: PeerId,
    pub network: DriaNetwork,
}

impl DriaRPC {
    /// Creates a new RPC target at the given type, along with a network type for refreshing the RPC address.
    pub fn new(addr: Multiaddr, network: DriaNetwork) -> Result<Self> {
        let peer_id = addr
            .iter()
            .find_map(|p| match p {
                Protocol::P2p(peer_id) => Some(peer_id),
                _ => None,
            })
            .ok_or_eyre("did not find peer ID within the returned RPC address")?;

        Ok(Self {
            addr,
            peer_id,
            network,
        })
    }

    /// Creates a new RPC target for the given network type and version.
    pub async fn new_for_network(network: DriaNetwork, version: &SemanticVersion) -> Result<Self> {
        let addr = get_rpc_for_network(&network, version).await?;
        Self::new(addr, network)
    }
}

/// Calls the DKN API to get an RPC address for the given network type.
///
/// The peer id is expected to be within the multi-address.
async fn get_rpc_for_network(
    network: &DriaNetwork,
    version: &SemanticVersion,
) -> Result<Multiaddr> {
    let response = reqwest::get(network.discovery_url(version)).await?;
    let rpcs_and_peer_counts = response
        .json::<Vec<(Multiaddr, usize)>>()
        .await
        .wrap_err("could not parse API response")?;

    // returns the RPC address with the least peer count (for load balancing)
    rpcs_and_peer_counts
        .into_iter()
        .min_by_key(|(_, peer_count)| *peer_count)
        .ok_or_eyre("no RPCs were returned by discovery API")
        .map(|(addr, _)| addr)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_dria_nodes() {
        let node =
            DriaRPC::new_for_network(DriaNetwork::Mainnet, &SemanticVersion::from_crate_version())
                .await;
        assert!(node.is_ok());
    }

    #[test]
    fn test_deserialize() {
        let input = r#"[
          ["/ip4/12.34.56.78/tcp/4001/p2p/16Uiu2HAmG7qrpSh8kenjuYqyrwxgEVdzqRV4wM1hHAZRq4j25VBC", 1],
          ["/ip4/78.56.34.12/tcp/4001/p2p/16Uiu2HAmG7qrpSh8kenjuYqyrwxgEVdzqRV4wM1hHAZRq4j25VBC", 4]
        ]"#;
        let result: Vec<(Multiaddr, usize)> = serde_json::from_str(input).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, 1);
        assert_eq!(result[1].1, 4);
    }
}
