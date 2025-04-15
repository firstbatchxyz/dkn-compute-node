use dkn_p2p::libp2p::{multiaddr::Protocol, Multiaddr, PeerId};
use dkn_p2p::DriaNetworkType;
use eyre::{Context, OptionExt, Result};
use std::fmt::Debug;

/// The connected RPC node, as per the Star network topology.
#[derive(Debug, Clone)]
pub struct DriaRPC {
    pub addr: Multiaddr,
    pub peer_id: PeerId,
    pub network: DriaNetworkType,
}

impl DriaRPC {
    /// Creates a new RPC target at the given type, along with a network type for refreshing the RPC address.
    pub fn new(addr: Multiaddr, network: DriaNetworkType) -> Result<Self> {
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

    /// Creates a new RPC target for the given network type.
    pub async fn new_for_network(network: DriaNetworkType) -> Result<Self> {
        let addr = get_rpc_for_network(&network).await?;
        Self::new(addr, network)
    }
}

/// Calls the DKN API to get an RPC address for the given network type.
///
/// The peer id is expected to be within the multi-address.
async fn get_rpc_for_network(network: &DriaNetworkType) -> Result<Multiaddr> {
    #[derive(serde::Deserialize, Debug)]
    struct DriaNodesApiResponse {
        pub rpc: Multiaddr,
    }

    // url to be used is determined by the network type
    let url = match network {
        DriaNetworkType::Community => "https://dkn.dria.co/v5/available-nodes",
        DriaNetworkType::Pro => "https://dkn.dria.co/v5/sdk/available-nodes",
        DriaNetworkType::Test => "https://dkn.dria.co/v5/test/available-nodes",
    };

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
        let node = DriaRPC::new_for_network(DriaNetworkType::Community).await;
        assert!(node.is_ok());
    }
}
