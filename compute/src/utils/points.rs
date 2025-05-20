use dkn_utils::DriaNetwork;
use eyre::Context;

pub struct DriaPointsClient {
    pub url: String,
    client: reqwest::Client,
    /// The total number of points you have accumulated at the start of the run.
    pub initial: f64,
}

#[derive(Debug, serde::Deserialize)]
pub struct DriaPoints {
    /// Indicates in which top percentile your points are.
    pub percentile: usize,
    /// The total number of points you have accumulated.
    pub score: f64,
}

impl DriaPointsClient {
    /// The base URL for the points API, w.r.t network.
    pub fn base_url(network: &DriaNetwork) -> &'static str {
        match network {
            DriaNetwork::Mainnet => "https://mainnet.dkn.dria.co/points/v0/total/node/",
            DriaNetwork::Testnet => "https://testnet.dkn.dria.co/points/v0/total/node/",
        }
    }

    /// Creates a new `DriaPointsClient` for the given address.
    pub fn new(address: &str, network: &DriaNetwork) -> eyre::Result<Self> {
        const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

        let url = format!(
            "{}/0x{}",
            Self::base_url(network),
            address.trim_start_matches("0x")
        );

        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .wrap_err("could not create Points client")?;

        Ok(Self {
            url,
            client,
            initial: 0.0,
        })
    }

    /// Sets the initial points to the current points.
    ///
    /// If there is an error, it sets to 0.0.
    pub async fn initialize(&mut self) {
        self.initial = self.get_points().await.map(|p| p.score).unwrap_or_default();
    }

    pub async fn get_points(&self) -> eyre::Result<DriaPoints> {
        let res = self
            .client
            .get(&self.url)
            .send()
            .await
            .wrap_err("could not make request")?;
        res.json::<DriaPoints>()
            .await
            .wrap_err("could not parse response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_points() {
        let client = DriaPointsClient::new(
            "0xa43536a6032a3907ccf60e8109429ee1047b207c",
            &DriaNetwork::Mainnet,
        )
        .unwrap();
        let steps = client.get_points().await.unwrap();
        assert!(steps.score >= 0.0);
        assert!(steps.percentile <= 100);
    }
}
