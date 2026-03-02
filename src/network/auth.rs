use serde::{Deserialize, Serialize};

use crate::error::NodeError;
use crate::identity::Identity;
use crate::network::protocol::{Capacity, read_framed, write_framed};

// ---------------------------------------------------------------------------
// Auth protocol types (separate from NodeMessage/RouterMessage since auth
// happens before the main protocol phase)
// ---------------------------------------------------------------------------

/// Router sends this challenge after QUIC connection is established.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeMessage {
    pub challenge: [u8; 32],
}

/// Node responds with signed challenge + metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// Ethereum address (hex, no 0x prefix).
    pub address: String,
    /// Signature over SHA-256(challenge).
    pub signature: Vec<u8>,
    /// Recovery ID for the signature.
    pub recovery_id: u8,
    /// Models this node can serve.
    pub models: Vec<String>,
    /// Benchmark tokens-per-second.
    pub tps: f64,
    /// Node software version.
    pub version: String,
    /// Current capacity.
    pub capacity: Capacity,
}

/// Router responds with auth result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub authenticated: bool,
    /// Assigned node ID on success.
    pub node_id: Option<String>,
    /// Error message on failure.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------

/// Perform the authentication handshake on an already-opened bi-directional stream.
///
/// 1. Read `ChallengeMessage` from the router
/// 2. Sign the challenge with our identity
/// 3. Send `AuthRequest` with node metadata
/// 4. Read `AuthResponse` and return the assigned node_id
pub async fn authenticate(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    identity: &Identity,
    models: Vec<String>,
    tps: f64,
    capacity: Capacity,
) -> Result<String, NodeError> {
    // 1. Read challenge
    let challenge_msg: ChallengeMessage = read_framed(recv)
        .await?
        .ok_or_else(|| NodeError::Network("connection closed before challenge".into()))?;

    // 2. Sign challenge
    let (signature, recovery_id) = identity.sign(&challenge_msg.challenge);

    // 3. Send auth request
    let auth_req = AuthRequest {
        address: identity.address_hex.clone(),
        signature: signature.serialize().to_vec(),
        recovery_id: recovery_id.serialize(),
        models,
        tps,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capacity,
    };
    write_framed(send, &auth_req).await?;

    // 4. Read auth response
    let auth_resp: AuthResponse = read_framed(recv)
        .await?
        .ok_or_else(|| NodeError::Network("connection closed before auth response".into()))?;

    if auth_resp.authenticated {
        auth_resp
            .node_id
            .ok_or_else(|| NodeError::Network("auth succeeded but no node_id returned".into()))
    } else {
        Err(NodeError::Network(format!(
            "authentication failed: {}",
            auth_resp.error.unwrap_or_else(|| "unknown".into())
        )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_message_roundtrip() {
        let msg = ChallengeMessage {
            challenge: [0x42; 32],
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: ChallengeMessage = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(roundtrip.challenge, [0x42; 32]);
    }

    #[test]
    fn test_auth_request_roundtrip() {
        let req = AuthRequest {
            address: "deadbeef".into(),
            signature: vec![1, 2, 3],
            recovery_id: 0,
            models: vec!["gemma3:4b".into()],
            tps: 42.5,
            version: "2.0.0".into(),
            capacity: Capacity { free: 1, max: 2 },
        };
        let packed = rmp_serde::to_vec(&req).unwrap();
        let roundtrip: AuthRequest = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(roundtrip.address, "deadbeef");
        assert_eq!(roundtrip.models, vec!["gemma3:4b"]);
        assert!((roundtrip.tps - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_auth_response_success_roundtrip() {
        let resp = AuthResponse {
            authenticated: true,
            node_id: Some("node-123".into()),
            error: None,
        };
        let packed = rmp_serde::to_vec(&resp).unwrap();
        let roundtrip: AuthResponse = rmp_serde::from_slice(&packed).unwrap();
        assert!(roundtrip.authenticated);
        assert_eq!(roundtrip.node_id.unwrap(), "node-123");
    }

    #[test]
    fn test_auth_response_failure_roundtrip() {
        let resp = AuthResponse {
            authenticated: false,
            node_id: None,
            error: Some("bad signature".into()),
        };
        let packed = rmp_serde::to_vec(&resp).unwrap();
        let roundtrip: AuthResponse = rmp_serde::from_slice(&packed).unwrap();
        assert!(!roundtrip.authenticated);
        assert_eq!(roundtrip.error.unwrap(), "bad signature");
    }
}
