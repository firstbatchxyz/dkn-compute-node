use eyre::Context;
use serde::Deserialize;

/// Points URL, use with an `address` query parameter.
const POINTS_API_BASE_URL: &str = "https://dkn.dria.co/dashboard/supply/v0/leaderboard/steps";

#[derive(Debug, Deserialize)]
pub struct DriaPoints {
    #[serde(deserialize_with = "deserialize_percentile")]
    /// Indicates in which top percentile your points are.
    pub percentile: u64,
    /// The total number of points you have accumulated.
    pub score: f64,
}

// the API returns a stringified number due to frontend issues, so we need to parse it
fn deserialize_percentile<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    let parsed = s.parse().map_err(serde::de::Error::custom)?;

    if parsed > 100 {
        return Err(serde::de::Error::custom(
            "percentile must be between 0 and 100",
        ));
    }

    Ok(parsed)
}

/// Returns the points for the given address.
pub async fn get_points(address: &str) -> eyre::Result<DriaPoints> {
    // the address can have 0x or not, we add it ourselves here
    let url = format!(
        "{}?address=0x{}",
        POINTS_API_BASE_URL,
        address.trim_start_matches("0x")
    );

    reqwest::get(&url)
        .await
        .wrap_err("could not make request")?
        .json::<DriaPoints>()
        .await
        .wrap_err("could not parse body")
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
