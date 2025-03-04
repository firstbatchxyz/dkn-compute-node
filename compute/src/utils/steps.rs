use eyre::Context;
use serde::Deserialize;

const STEPS_API_BASE_URL: &str = "https://dkn.dria.co/dashboard/supply/v0/leaderboard/steps";

#[derive(Debug, Deserialize)]
pub struct StepsScore {
    #[serde(deserialize_with = "deserialize_percentile")]
    /// Indicates in which top percentile your steps are.
    pub percentile: u64,
    /// The total number of steps you have accumulated.
    pub score: u64,
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

/// Returns the steps for the given address.
pub async fn get_steps(address: &str) -> eyre::Result<StepsScore> {
    // the address can have 0x or not, we add it ourselves here
    let url = format!(
        "{}?address=0x{}",
        STEPS_API_BASE_URL,
        address.trim_start_matches("0x")
    );

    reqwest::get(&url)
        .await
        .wrap_err("could not make request")?
        .json::<StepsScore>()
        .await
        .wrap_err("could not parse steps body")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_steps() {
        let steps = get_steps("0xa43536a6032a3907ccf60e8109429ee1047b207c")
            .await
            .unwrap();
        println!("{:?}", steps);
    }
}
