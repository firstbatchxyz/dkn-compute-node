use serde::{Deserialize, Serialize};

/// A single token emitted during streaming generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToken {
    /// The decoded text of this token.
    pub text: String,
    /// The zero-based position of this token in the generated sequence.
    pub index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_token_serde() {
        let token = StreamToken {
            text: "hello".into(),
            index: 0,
        };
        let json = serde_json::to_string(&token).unwrap();
        let roundtrip: StreamToken = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.text, "hello");
        assert_eq!(roundtrip.index, 0);
    }

    #[test]
    fn test_stream_token_msgpack() {
        let token = StreamToken {
            text: "world".into(),
            index: 42,
        };
        let packed = rmp_serde::to_vec(&token).unwrap();
        let roundtrip: StreamToken = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(roundtrip.text, "world");
        assert_eq!(roundtrip.index, 42);
    }
}
