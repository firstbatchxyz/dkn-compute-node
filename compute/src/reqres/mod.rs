//! Request-response handlers.

use eyre::Context;
use serde::{de::DeserializeOwned, Serialize};

mod specs;
pub use specs::SpecResponder;

mod task;
pub use task::TaskResponder;

mod heartbeat;
pub use heartbeat::HeartbeatRequester;

/// A responder should implement a request & response type, both serializable.
///
/// The `try_parse_request` is automatically implemented using `serde-json` for a byte slice.
pub trait IsResponder {
    type Request: DeserializeOwned;
    type Response: Serialize + DeserializeOwned;

    fn try_parse_request(data: &[u8]) -> eyre::Result<Self::Request> {
        serde_json::from_slice(data).wrap_err("could not parse request")
    }

    fn try_parse_response(data: &[u8]) -> eyre::Result<Self::Response> {
        serde_json::from_slice(data).wrap_err("could not parse response")
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // TODO: remove this test when we migrate to enum-based bodies
    #[test]
    fn test_enum_serialization() {
        use serde::Deserialize;
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct AEnum {
            a1: bool,
            a2: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct BEnum {
            b1: u64,
            b2: bool,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        #[serde(tag = "type", rename_all = "camelCase")]
        enum TestEnum {
            A(AEnum),
            B(BEnum),
        }

        let a_variant = TestEnum::A(AEnum {
            a1: true,
            a2: "test".to_string(),
        });
        let b_variant = TestEnum::B(BEnum {
            b1: 123456789,
            b2: false,
        });

        let a_serialized = serde_json::to_string(&a_variant).unwrap();
        let b_serialized = serde_json::to_string(&b_variant).unwrap();

        assert_eq!(a_serialized, r#"{"type":"a","a1":true,"a2":"test"}"#);
        assert_eq!(b_serialized, r#"{"type":"b","b1":123456789,"b2":false}"#);

        let a_deserialized: TestEnum = serde_json::from_str(&a_serialized).unwrap();
        let b_deserialized: TestEnum = serde_json::from_str(&b_serialized).unwrap();

        assert_eq!(a_variant, a_deserialized);
        assert_eq!(b_variant, b_deserialized);
    }
}
