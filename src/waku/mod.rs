pub mod message;
mod relay;

const DEFAULT_WAKU_URL: &str = "http://127.0.0.1:8645";

use std::env;

use crate::errors::NodeResult;

use crate::utils::http::BaseClient;

use self::relay::RelayClient;
use serde::{Deserialize, Serialize};

/// Waku [REST API](https://waku-org.github.io/waku-rest-api) wrapper.
#[derive(Debug, Clone)]
pub struct WakuClient {
    base: BaseClient,
    pub relay: RelayClient,
}

impl Default for WakuClient {
    fn default() -> Self {
        WakuClient::new(None)
    }
}

impl WakuClient {
    /// Creates a new instance of WakuClient.
    pub fn new(url: Option<String>) -> Self {
        let url: String =
            url.unwrap_or_else(|| env::var("WAKU_URL").unwrap_or(DEFAULT_WAKU_URL.to_string()));
        log::info!("Waku URL: {}", url);

        let base = BaseClient::new(url);
        let relay = RelayClient::new(base.clone());
        WakuClient { base, relay }
    }

    /// Health-check for the node.
    pub async fn health(&self) -> NodeResult<(bool, String)> {
        let res = self.base.get("health", None).await?;
        let msg = res.text().await?;
        Ok((msg == "Node is healthy", msg))
    }

    /// Returns the node version as `vX.X.X`.
    pub async fn version(&self) -> NodeResult<String> {
        let res = self.base.get("debug/v1/version", None).await?;
        let version = res.text().await?;
        Ok(version)
    }

    /// Returns debug information.
    pub async fn info(&self) -> NodeResult<InfoResponse> {
        let res = self.base.get("debug/v1/info", None).await?;
        let info = res.json().await?;
        Ok(info)
    }

    /// Returns the connected peers.
    pub async fn peers(&self) -> NodeResult<Vec<PeerInfo>> {
        let res = self.base.get("admin/v1/peers", None).await?;
        let peers = res.json().await?;
        Ok(peers)
    }
}

/// Debug information response.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub listen_addresses: Vec<String>,
    pub enr_uri: String,
}

/// Peer information.
#[derive(Serialize, Deserialize, Debug)]
pub struct PeerInfo {
    pub multiaddr: String,
    pub protocols: Vec<ProtocolInfo>,
}

/// Protocol information.
#[derive(Serialize, Deserialize, Debug)]
pub struct ProtocolInfo {
    pub protocol: String,
    pub connected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waku_config() {
        env::set_var("WAKU_URL", "im-a-host:1337");

        let waku = WakuClient::new(None);
        assert_eq!(waku.base.get_base_url(), "im-a-host:1337");
    }
}
