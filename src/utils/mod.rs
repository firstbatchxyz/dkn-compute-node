pub mod message;
use std::collections::HashMap;
use std::time::SystemTime;
use url::form_urlencoded;

/// A [Content Topic](https://docs.waku.org/learn/concepts/content-topics) is represented as a string with the form:
///
/// ```sh
/// /app-name/version/content-topic/encoding
/// /waku/2/default-waku/proto # example
/// /my-app/2/chatroom-1/proto # example
/// ```
///
/// In our case, `version` is always 0 (no encryption at protocol layer) and `encoding` is always `proto` (protobuf).
///
/// `app-name` defaults to `dria` unless specified otherwise with the second argument.
///
pub fn create_content_topic(topic: String, app: Option<String>) -> String {
    let app = app.unwrap_or("dria".to_string());

    format!("%2F{}%2F0%2F{}%2Fproto", app, topic)
}

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
    fn test_create_content_topic() {
        let topic = "default-waku".to_string();

        let app = "waku".to_string();
        let expected = "%2Fwaku%2F0%2Fdefault-waku%2Fproto".to_string();
        assert_eq!(create_content_topic(topic.clone(), Some(app)), expected);

        let expected = "%2Fdria%2F0%2Fdefault-waku%2Fproto".to_string();
        assert_eq!(create_content_topic(topic, None), expected);
    }

    #[test]
    fn test_convert_to_query_params() {
        let mut params = HashMap::new();
        params.insert("key1".to_string(), "value/1".to_string());
        params.insert("key2".to_string(), "value_2".to_string());
        let expected = "key2=value_2&key1=value%2F1".to_string();
        assert_eq!(convert_to_query_params(params), expected);
    }
}
