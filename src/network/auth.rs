use std::collections::HashMap;

use crate::error::NodeError;
use crate::identity::Identity;
use crate::network::protocol::{Capacity, read_framed, write_framed};

pub use dkn_protocol::{AuthRequest, AuthResponse, ChallengeMessage};

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
    tps: HashMap<String, f64>,
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
