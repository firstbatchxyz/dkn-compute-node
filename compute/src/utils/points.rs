use eyre::Context;

/// Points URL, use with an `address` query parameter.
const POINTS_API_BASE_URL: &str =
    "https://mainnet.dkn.dria.co/dashboard/supply/v0/leaderboard/steps";

#[derive(Debug, serde::Deserialize)]
pub struct DriaPoints {
    /// Indicates in which top percentile your points are.
    pub percentile: u32,
    /// The total number of points you have accumulated.
    pub score: f64,
}

/// Returns the points for the given address.
pub async fn get_points(address: &str) -> eyre::Result<DriaPoints> {
    // the address can have 0x or not, we add it ourselves here
    let url = format!(
        "{}?address=0x{}",
        POINTS_API_BASE_URL,
        address.trim_start_matches("0x")
    );

    let res = reqwest::get(&url)
        .await
        .wrap_err("could not make request")?;
    res.json::<DriaPoints>()
        .await
        .wrap_err("could not parse response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_points() {
        let steps = get_points("0xa43536a6032a3907ccf60e8109429ee1047b207c")
            .await
            .unwrap();
        assert!(steps.score != 0.0);
    }
}
