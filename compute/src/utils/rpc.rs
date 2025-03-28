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
    /// Creates a new `AvailableNodes` struct for the given network type.
    ///
    /// Will panic if anything goes wrong.
    pub async fn new(network: DriaNetworkType) -> Result<Self> {
        let addr = refresh_rpc_addr(&network).await?;
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
}

/// Calls the DKN API to get an RPC address for the given network type.
///
/// The peer id is expected to be within the multi-address.
async fn refresh_rpc_addr(network: &DriaNetworkType) -> Result<Multiaddr> {
    #[derive(serde::Deserialize, Debug)]
    struct DriaNodesApiResponse {
        pub rpc: Multiaddr,
    }

    // url to be used is determined by the network type
    let url = match network {
        DriaNetworkType::Community => "https://dkn.dria.co/v4/available-nodes",
        DriaNetworkType::Pro => "https://dkn.dria.co/v4/sdk/available-nodes",
        DriaNetworkType::Test => "https://dkn.dria.co/v4/test/available-nodes",
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
        let node = DriaRPC::new(DriaNetworkType::Community).await;
        assert!(node.is_ok());
    }
}
