#![allow(unused)]

use super::{base::BaseClient, relay::RelayClient, store::StoreClient};
use serde::{Deserialize, Serialize};

/// Waku [REST API](https://waku-org.github.io/waku-rest-api) wrapper.
#[derive(Debug, Clone)]
pub struct WakuClient {
    base: BaseClient,
    pub store: StoreClient,
    pub relay: RelayClient,
}

impl Default for WakuClient {
    fn default() -> Self {
        WakuClient::new("http://127.0.0.1:8645")
    }
}

impl WakuClient {
    /// Creates a new instance of WakuClient.
    pub fn new(base_url: &str) -> Self {
        let base = BaseClient::new(base_url);
        let store = StoreClient::new(base.clone());
        let relay = RelayClient::new(base.clone());

        WakuClient { base, store, relay }
    }

    /// Health-check for the node.
    pub async fn health(&self) -> Result<(bool, String), Box<dyn std::error::Error>> {
        let res = self.base.get("health", None).await?;
        let msg = res.text().await?;
        Ok((msg == "Node is healthy", msg))
    }

    /// Returns the node version as `vX.X.X`.
    pub async fn version(&self) -> Result<String, Box<dyn std::error::Error>> {
        let res = self.base.get("debug/v1/version", None).await?;
        let version = res.text().await?;
        Ok(version)
    }

    /// Returns debug information.
    pub async fn info(&self) -> Result<InfoResponse, Box<dyn std::error::Error>> {
        let res = self.base.get("debug/v1/info", None).await?;
        let info = res.json().await?;
        Ok(info)
    }

    /// Returns the connected peers.
    pub async fn peers(&self) -> Result<Vec<PeerInfo>, Box<dyn std::error::Error>> {
        let res = self.base.get("admin/v1/peers", None).await?;
        let peers = res.json().await?;
        Ok(peers)
    }
}

/// Debug information response.
#[derive(Serialize, Deserialize)]
pub struct InfoResponse {
    pub listen_addresses: Vec<String>,
    pub enr_uri: String,
}

/// Peer information.
#[derive(Serialize, Deserialize)]
pub struct PeerInfo {
    pub multi_addr: String,
    pub protocols: Vec<ProtocolInfo>,
}

/// Protocol information.
#[derive(Serialize, Deserialize)]
pub struct ProtocolInfo {
    pub protocol: String,
    pub connected: bool,
}

#[cfg(feature = "waku-test")]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version() {
        let waku = WakuClient::default();
        let version = waku.version().await.unwrap();
        assert_eq!("v0.26.0", version);

        // relayed
        // let msgs = waku
        //     .relay
        //     .get_messages("/dria/1/synthesis/protobuf")
        //     .await
        //     .unwrap();
        // println!("Messages: {:?}", msgs);

        // stored
        // let msgs = waku
        //     .store
        //     .get_messages("/dria/1/synthesis/protobuf", Some(true), None)
        //     .await
        //     .unwrap();
        // println!("Messages: {:?}", msgs);
    }
}
