use dkn_p2p::{libp2p::PeerId, DriaNetworkType, DriaNodes};
use dkn_utils::parse_vec;
use eyre::Result;

/// Refresh available nodes using the API.
pub async fn refresh_dria_nodes(nodes: &mut DriaNodes) -> Result<()> {
    #[derive(serde::Deserialize, Debug)]
    struct AvailableNodesApiResponse {
        pub bootstraps: Vec<String>,
        pub relays: Vec<String>,
        pub rpcs: Vec<String>,
        #[serde(rename = "rpcAddrs")]
        pub rpc_addrs: Vec<String>,
    }

    // url to be used is determined by the network type
    let url = match nodes.network {
        DriaNetworkType::Community => "https://dkn.dria.co/available-nodes",
        DriaNetworkType::Pro => "https://dkn.dria.co/sdk/available-nodes",
        DriaNetworkType::Test => "https://dkn.dria.co/test/available-nodes",
    };

    // make the request
    let response = reqwest::get(url).await?;
    let response_body = response.json::<AvailableNodesApiResponse>().await?;
    nodes
        .bootstrap_nodes
        .extend(parse_vec(response_body.bootstraps).unwrap_or_else(|e| {
            log::error!("Failed to parse bootstrap nodes: {}", e);
            vec![]
        }));
    nodes
        .relay_nodes
        .extend(parse_vec(response_body.relays).unwrap_or_else(|e| {
            log::error!("Failed to parse relay nodes: {}", e);
            vec![]
        }));
    nodes
        .rpc_nodes
        .extend(parse_vec(response_body.rpc_addrs).unwrap_or_else(|e| {
            log::error!("Failed to parse rpc nodes: {}", e);
            vec![]
        }));
    nodes
        .rpc_peerids
        .extend(parse_vec::<PeerId>(response_body.rpcs).unwrap_or_else(|e| {
            log::error!("Failed to parse rpc peerids: {}", e);
            vec![]
        }));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_refresh_dria_nodes() {
        let mut nodes = DriaNodes::new(DriaNetworkType::Community);
        refresh_dria_nodes(&mut nodes).await.unwrap();
        println!("Community: {:#?}", nodes);

        let mut nodes = DriaNodes::new(DriaNetworkType::Pro);
        refresh_dria_nodes(&mut nodes).await.unwrap();
        println!("Pro: {:#?}", nodes);
    }
}
