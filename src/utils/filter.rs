use fastbloom_rs::{BloomFilter, Hashes};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

/// A task filter is used to determine if a node is selected.
///
/// The filter is a Bloom Filter with a set of items and a false positive rate, it is serialized as a hex string.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilterPayload {
    pub filter: String,
    pub hashes: u32,
}

impl From<FilterPayload> for String {
    fn from(value: FilterPayload) -> Self {
        to_string(&json!(value)).unwrap() // TODO: handle error
    }
}

impl From<String> for FilterPayload {
    fn from(value: String) -> Self {
        serde_json::from_str(value.as_str()).expect("Could not parse FilterPayload")
    }
}

impl From<FilterPayload> for BloomFilter {
    fn from(value: FilterPayload) -> Self {
        let filter = hex::decode(value.filter).unwrap(); // TODO: handle error
        BloomFilter::from_u8_array(filter.as_slice(), value.hashes)
    }
}

impl From<BloomFilter> for FilterPayload {
    fn from(value: BloomFilter) -> Self {
        FilterPayload {
            filter: hex::encode(value.get_u8_array()),
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
            filter: FILTER_HEX.to_string(),
            hashes: 7,
        };

        let bf = BloomFilter::from(filter_payload);
        assert!(bf.contains(b"helloworld"));
        assert!(!bf.contains(b"im not in this filter"));
    }

    #[test]
    fn test_filter_read_2() {
        // 128 items, 0.01 fp rate (7 hashes), includes b"helloworld" and nothing else
        const FILTER_HEX: &str = "00000040000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000004000000000000000000000000000000000000000000000040000000000000000000000000004000000000000000000000000000000000000";
        let filter_payload = FilterPayload {
            filter: FILTER_HEX.to_string(),
            hashes: 7,
        };

        let bf = BloomFilter::from(filter_payload);
        assert!(bf.contains(b"helloworld"));
        assert!(!bf.contains(b"im not in this filter"));
    }
}
