use eyre::Context;

/// Points URL, use with an `address` query parameter.
const POINTS_API_BASE_URL: &str =
    "https://mainnet.dkn.dria.co/dashboard/supply/v0/leaderboard/steps";
// TODO: support testnet here?

pub struct DriaPointsClient {
    pub url: String,
    client: reqwest::Client,
    /// The total number of points you have accumulated at the start of the run.
    pub initial: f64,
}

impl DriaPointsClient {
    /// Creates a new `DriaPointsClient` for the given address.
    pub fn new(address: &str) -> eyre::Result<Self> {
        const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

        let url = format!(
            "{}?address=0x{}",
            POINTS_API_BASE_URL,
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

#[derive(Debug, serde::Deserialize)]
pub struct DriaPoints {
    /// Indicates in which top percentile your points are.
    ///
    /// TODO: can be number in API
    /// TODO: API sometimes returns `null` here?
    pub percentile: Option<String>,
    /// The total number of points you have accumulated.
    pub score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "waiting for API"]
    async fn test_get_points() {
        let client = DriaPointsClient::new("0xa43536a6032a3907ccf60e8109429ee1047b207c").unwrap();
        let steps = client.get_points().await.unwrap();
        assert!(steps.score != 0.0);
    }
}
