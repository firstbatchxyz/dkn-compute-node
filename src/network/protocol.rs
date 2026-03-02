use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::NodeError;
use crate::models::template::ChatMessage;

// ---------------------------------------------------------------------------
// Node → Router messages
// ---------------------------------------------------------------------------

/// Messages sent from this compute node to the router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeMessage {
    /// Completed inference result for a task.
    TaskResult {
        task_id: Uuid,
        text: String,
        stats: TaskStats,
        proof: Option<crate::inference::InferenceProof>,
    },
    /// We cannot accept the assigned task.
    TaskRejected {
        task_id: Uuid,
        reason: RejectReason,
    },
    /// Periodic or on-demand status snapshot.
    StatusUpdate {
        models: Vec<String>,
        capacity: Capacity,
        version: String,
    },
    /// Response to a router challenge (placeholder).
    ChallengeResponse {
        challenge: [u8; 32],
        signature: Vec<u8>,
        recovery_id: u8,
    },
}

// ---------------------------------------------------------------------------
// Router → Node messages
// ---------------------------------------------------------------------------

/// Messages sent from the router to this compute node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouterMessage {
    /// A new inference task to execute.
    TaskAssignment {
        task_id: Uuid,
        model: String,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        temperature: f32,
        validation: Option<ValidationRequest>,
    },
    /// Challenge for proof-of-liveness.
    Challenge { challenge: [u8; 32] },
    /// Heartbeat / keep-alive ping.
    Ping,
    /// Updated model registry from the router.
    ModelRegistryUpdate {
        entries: Vec<ModelRegistryEntry>,
    },
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Statistics about a completed inference task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStats {
    pub tokens_generated: u32,
    pub prompt_tokens: u32,
    pub generation_time_ms: u64,
    pub tokens_per_second: f64,
}

/// Reason a task was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RejectReason {
    /// Model not loaded on this node.
    ModelNotLoaded,
    /// All inference slots are busy.
    AtCapacity,
    /// Task parameters are invalid.
    InvalidRequest(String),
}

/// Current capacity snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capacity {
    /// Number of free inference slots.
    pub free: usize,
    /// Maximum concurrent inference slots.
    pub max: usize,
}

/// Optional validation parameters included with a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    /// Token positions at which to extract logprobs.
    pub logprob_positions: Vec<usize>,
    /// Top-k alternatives to collect at each logprob position.
    pub logprob_top_k: usize,
}

/// A model entry from the router's registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryEntry {
    pub name: String,
    pub hf_repo: String,
    pub hf_file: String,
    pub chat_template: Option<String>,
}

// ---------------------------------------------------------------------------
// Length-prefixed MessagePack framing
// ---------------------------------------------------------------------------

/// Maximum allowed message size (16 MB).
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Write a length-prefixed MessagePack message to a QUIC send stream.
///
/// Wire format: `[4-byte BE length][msgpack payload]`
pub async fn write_framed<T: Serialize>(
    send: &mut quinn::SendStream,
    msg: &T,
) -> Result<(), NodeError> {
    let payload =
        rmp_serde::to_vec(msg).map_err(|e| NodeError::Network(format!("serialize: {e}")))?;
    let len = payload.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(NodeError::Network(format!(
            "message too large: {len} bytes (max {MAX_MESSAGE_SIZE})"
        )));
    }
    send.write_all(&len.to_be_bytes())
        .await
        .map_err(|e| NodeError::Network(format!("write length: {e}")))?;
    send.write_all(&payload)
        .await
        .map_err(|e| NodeError::Network(format!("write payload: {e}")))?;
    Ok(())
}

/// Read a length-prefixed MessagePack message from a QUIC receive stream.
///
/// Returns `Ok(None)` on clean EOF (stream closed), `Err` on protocol violations.
pub async fn read_framed<T: serde::de::DeserializeOwned>(
    recv: &mut quinn::RecvStream,
) -> Result<Option<T>, NodeError> {
    let mut len_buf = [0u8; 4];
    match recv.read_exact(&mut len_buf).await {
        Ok(()) => {}
        Err(quinn::ReadExactError::FinishedEarly(_)) => return Ok(None),
        Err(e) => return Err(NodeError::Network(format!("read length: {e}"))),
    }
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_MESSAGE_SIZE {
        return Err(NodeError::Network(format!(
            "message too large: {len} bytes (max {MAX_MESSAGE_SIZE})"
        )));
    }
    let mut payload = vec![0u8; len as usize];
    recv.read_exact(&mut payload)
        .await
        .map_err(|e| NodeError::Network(format!("read payload: {e}")))?;
    let msg = rmp_serde::from_slice(&payload)
        .map_err(|e| NodeError::Network(format!("deserialize: {e}")))?;
    Ok(Some(msg))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_message_roundtrip() {
        let msg = NodeMessage::TaskResult {
            task_id: Uuid::nil(),
            text: "Hello world".into(),
            stats: TaskStats {
                tokens_generated: 10,
                prompt_tokens: 5,
                generation_time_ms: 100,
                tokens_per_second: 100.0,
            },
            proof: None,
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: NodeMessage = rmp_serde::from_slice(&packed).unwrap();
        match roundtrip {
            NodeMessage::TaskResult { task_id, text, .. } => {
                assert_eq!(task_id, Uuid::nil());
                assert_eq!(text, "Hello world");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_router_message_roundtrip() {
        let msg = RouterMessage::TaskAssignment {
            task_id: Uuid::nil(),
            model: "gemma3:4b".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            max_tokens: 512,
            temperature: 0.7,
            validation: None,
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: RouterMessage = rmp_serde::from_slice(&packed).unwrap();
        match roundtrip {
            RouterMessage::TaskAssignment { model, .. } => {
                assert_eq!(model, "gemma3:4b");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_reject_reason_roundtrip() {
        let msg = NodeMessage::TaskRejected {
            task_id: Uuid::nil(),
            reason: RejectReason::AtCapacity,
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: NodeMessage = rmp_serde::from_slice(&packed).unwrap();
        match roundtrip {
            NodeMessage::TaskRejected { reason, .. } => {
                matches!(reason, RejectReason::AtCapacity);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_status_update_roundtrip() {
        let msg = NodeMessage::StatusUpdate {
            models: vec!["gemma3:4b".into()],
            capacity: Capacity { free: 2, max: 4 },
            version: "2.0.0".into(),
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: NodeMessage = rmp_serde::from_slice(&packed).unwrap();
        match roundtrip {
            NodeMessage::StatusUpdate {
                capacity, version, ..
            } => {
                assert_eq!(capacity.free, 2);
                assert_eq!(capacity.max, 4);
                assert_eq!(version, "2.0.0");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_challenge_roundtrip() {
        let msg = RouterMessage::Challenge {
            challenge: [0xAB; 32],
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: RouterMessage = rmp_serde::from_slice(&packed).unwrap();
        match roundtrip {
            RouterMessage::Challenge { challenge } => {
                assert_eq!(challenge, [0xAB; 32]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_ping_roundtrip() {
        let packed = rmp_serde::to_vec(&RouterMessage::Ping).unwrap();
        let roundtrip: RouterMessage = rmp_serde::from_slice(&packed).unwrap();
        assert!(matches!(roundtrip, RouterMessage::Ping));
    }

    #[test]
    fn test_model_registry_update_roundtrip() {
        let msg = RouterMessage::ModelRegistryUpdate {
            entries: vec![ModelRegistryEntry {
                name: "test:1b".into(),
                hf_repo: "repo/model".into(),
                hf_file: "model.gguf".into(),
                chat_template: Some("chatml".into()),
            }],
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        let roundtrip: RouterMessage = rmp_serde::from_slice(&packed).unwrap();
        match roundtrip {
            RouterMessage::ModelRegistryUpdate { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].name, "test:1b");
            }
            _ => panic!("wrong variant"),
        }
    }

    /// Test framing over a quinn duplex (uses tokio::io::duplex via quinn test helpers).
    /// Since we can't easily create a quinn stream in unit tests, test the serialization
    /// logic directly and verify size limits.
    #[test]
    fn test_message_size_within_limit() {
        let msg = NodeMessage::TaskResult {
            task_id: Uuid::nil(),
            text: "x".repeat(1000),
            stats: TaskStats {
                tokens_generated: 100,
                prompt_tokens: 50,
                generation_time_ms: 500,
                tokens_per_second: 200.0,
            },
            proof: None,
        };
        let packed = rmp_serde::to_vec(&msg).unwrap();
        assert!((packed.len() as u32) < MAX_MESSAGE_SIZE);
    }
}
