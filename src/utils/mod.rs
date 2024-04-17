pub mod crypto;
pub mod message;

use std::collections::HashMap;
use std::time::SystemTime;
use url::form_urlencoded;

/// Returns the current time in nanoseconds since the Unix epoch.
pub fn get_current_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

pub fn convert_to_query_params(params: HashMap<String, String>) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_query_params() {
        let mut params = HashMap::new();
        params.insert("key1".to_string(), "v_a lue/1".to_string());
        // we could test with multiple keys as well, but the ordering of parameters
        // may change sometimes which causes the test to fail randomly

        let expected = "key1=v_a+lue%2F1".to_string();
        assert_eq!(convert_to_query_params(params), expected);
    }
}
