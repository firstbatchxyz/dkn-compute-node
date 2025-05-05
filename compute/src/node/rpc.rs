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
    #[derive(serde::Deserialize, Debug)]
    struct DriaNodesApiResponse {
        pub rpc: Multiaddr,
    }

    // url to be used is determined by the network type
    let base_url = match network {
        DriaNetwork::Mainnet => "https://dkn.dria.co/available-nodes",
        DriaNetwork::Testnet => "https://dkn.dria.co/available-nodes",
    };
    let url = format!("{}/{}", base_url, version.as_major_minor());

    // make the request
    let response = reqwest::get(url).await?;
    let response_body = response
        .json::<DriaNodesApiResponse>()
        .await
        .wrap_err("could not parse API response")?;

    Ok(response_body.rpc)
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
}
