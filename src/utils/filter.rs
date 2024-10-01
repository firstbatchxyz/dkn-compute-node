use eyre::{Context, Result};
use fastbloom_rs::{BloomFilter, Hashes, Membership};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

/// A task filter is used to determine if a node is selected.
///
/// The filter is a Bloom Filter with a set of items and a false positive rate, it is serialized as a hex string.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilterPayload {
    pub(crate) hex: String,
    pub(crate) hashes: u32,
}

impl FilterPayload {
    /// Shorthand function to create the underlying `BloomFilter` and check if it contains the given address.
    pub fn contains(&self, address: &[u8]) -> Result<bool> {
        BloomFilter::try_from(self)
            .map(|filter| filter.contains(address))
            .wrap_err("Could not create filter.")
    }
}

// FIXME: too many TryFrom's here, simplify in a single function here!

impl TryFrom<&FilterPayload> for String {
    type Error = serde_json::Error;

    fn try_from(value: &FilterPayload) -> Result<Self, Self::Error> {
        let string = to_string(&json!(value))?;
        Ok(string)
    }
}

impl TryFrom<String> for FilterPayload {
    type Error = serde_json::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let payload = serde_json::from_str(value.as_str())?;
        Ok(payload)
    }
}

impl TryFrom<&FilterPayload> for BloomFilter {
    type Error = hex::FromHexError;

    fn try_from(value: &FilterPayload) -> Result<Self, Self::Error> {
        let filter = hex::decode(value.hex.as_str())?;
        Ok(BloomFilter::from_u8_array(&filter, value.hashes))
    }
}

impl From<BloomFilter> for FilterPayload {
    fn from(value: BloomFilter) -> Self {
        FilterPayload {
            hex: hex::encode(value.get_u8_array()),
            hashes: value.hashes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastbloom_rs::{FilterBuilder, Membership};

    #[test]
    fn test_bloom_filter() {
        let mut bloom = FilterBuilder::new(128, 0.01).build_bloom_filter();
        bloom.add(b"hello world!");
        assert!(bloom.contains(b"hello world!"));
        assert!(!bloom.contains(b"byebye world"));
    }

    #[test]
    fn test_filter_read_1() {
        // 250 items, 0.01 fp rate (7 hashes), includes b"helloworld" and nothing else
        const FILTER_HEX: &str = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000040000000000000400000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000004000000000000040000";
        let filter_payload = FilterPayload {
            hex: FILTER_HEX.to_string(),
            hashes: 7,
        };

        let bf = BloomFilter::try_from(&filter_payload).expect("Should parse filter");
        assert!(bf.contains(b"helloworld"));
        assert!(!bf.contains(b"im not in this filter"));
    }

    #[test]
    fn test_filter_read_2() {
        // 128 items, 0.01 fp rate (7 hashes), includes b"helloworld" and nothing else
        const FILTER_HEX: &str = "00000040000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000004000000000000000000000000000000000000000000000040000000000000000000000000004000000000000000000000000000000000000";
        let filter_payload = FilterPayload {
            hex: FILTER_HEX.to_string(),
            hashes: 7,
        };

        let bf = BloomFilter::try_from(&filter_payload).expect("Should parse filter");
        assert!(bf.contains(b"helloworld"));
        assert!(!bf.contains(b"im not in this filter"));
    }

    #[test]
    #[ignore = "this panics, its a bug within the filter library"]
    fn test_filter_empty() {
        let filter_payload = FilterPayload {
            hex: "".to_string(),
            hashes: 0,
        };

        BloomFilter::try_from(&filter_payload).expect("Should parse filter");
    }
}
