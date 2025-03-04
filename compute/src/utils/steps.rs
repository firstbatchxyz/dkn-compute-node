use eyre::Context;
use serde::{Deserialize, Serialize};

const STEPS_API_BASE_URL: &str = "https://dkn.dria.co/dashboard/supply/v0/leaderboard/steps";

#[derive(Debug, Serialize, Deserialize)]
pub struct StepsScore {
    /// Indicates in which top percentile your steps are.
    pub percentile: String,
    /// The total number of steps you have accumulated.
    pub score: u64,
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
