use serde::{Deserialize, Serialize};

/// Log-probability information for a single token position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogprob {
    /// Position in the generated sequence.
    pub position: usize,
    /// The token ID chosen at this position.
    pub token_id: u32,
    /// The decoded text of the chosen token.
    pub token_text: String,
    /// The log-probability of the chosen token.
    pub logprob: f32,
    /// Top-k alternatives: (token_text, logprob).
    pub top_k: Vec<(String, f32)>,
}

/// Proof-of-inference data for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceProof {
    /// Log-probabilities at requested positions.
    pub logprobs: Vec<TokenLogprob>,
    /// Optional KV-cache hash for determinism verification.
    /// Placeholder: currently hashes logits at probed position.
    pub kv_cache_hash: Option<[u8; 32]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_logprob_serde() {
        let lp = TokenLogprob {
            position: 5,
            token_id: 1234,
            token_text: "the".into(),
            logprob: -0.5,
            top_k: vec![("the".into(), -0.5), ("a".into(), -1.2)],
        };
        let json = serde_json::to_string(&lp).unwrap();
        let roundtrip: TokenLogprob = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.position, 5);
        assert_eq!(roundtrip.token_id, 1234);
        assert_eq!(roundtrip.top_k.len(), 2);
    }

    #[test]
    fn test_inference_proof_serde() {
        let proof = InferenceProof {
            logprobs: vec![TokenLogprob {
                position: 0,
                token_id: 1,
                token_text: "hello".into(),
                logprob: -0.1,
                top_k: vec![],
            }],
            kv_cache_hash: Some([0xAB; 32]),
        };
        let packed = rmp_serde::to_vec(&proof).unwrap();
        let roundtrip: InferenceProof = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(roundtrip.logprobs.len(), 1);
        assert!(roundtrip.kv_cache_hash.is_some());
    }
}
