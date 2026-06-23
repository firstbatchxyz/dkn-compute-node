//! Node-to-node hidden-state transport for the shard chain.
//!
//! This is the missing primitive of the whole project: today everything is star topology
//! (node ↔ router); pipeline serving needs a node ↔ node data link. The residual-stream
//! hidden state crosses each shard boundary here.
//!
//! STATUS: interface stub. The implementation reuses the existing quinn/QUIC stack
//! (`crate::network`) rather than introducing gRPC — see `docs/substrate-decision.md` and
//! the chain-formation / transport sections of `docs/pipeline-specdecode-design.md` and
//! `docs/chain-formation-design.md`. NAT traversal is router-coordinated hole-punching
//! with a relay fallback.

// Interface stub: these wire types are defined ahead of the transport implementation.
#![allow(dead_code)]

use super::engine::TokenId;

/// A residual-stream hidden state crossing a shard boundary: logically `[n_tokens, hidden]`.
/// Bandwidth is small (~12 KiB/token at H=6144, bf16) — the wire is RTT-bound, not
/// byte-bound — so the activation codec (`dtype`) is a second-order optimization for thin
/// home uplinks, not the latency lever.
#[derive(Clone, Debug)]
pub struct HiddenStateFrame {
    pub request_id: u64,
    pub chain_id: u64,
    /// Bumped on any chain re-plan; a node drops frames with a stale epoch (fencing).
    pub epoch: u64,
    pub step: u64,
    /// Which layer-block boundary this hidden state sits at (the cut index).
    pub layer_boundary: u32,
    pub n_tokens: u32,
    pub hidden_dim: u32,
    /// Activation codec on the wire, e.g. "bf16" | "fp8" | "q8_block".
    pub dtype: String,
    pub data: Vec<u8>,
}

/// The tail returns this to node-0 (direct return, one hop) for spec-decode verification:
/// the target's greedy argmax at each of the `gamma + 1` verify positions. (node-0 owns
/// `lm_head` + the draft, so acceptance logits stay numerically consistent with the draft;
/// for true sampling later this carries logprobs instead — see design docs.)
#[derive(Clone, Debug)]
pub struct VerifyResult {
    pub request_id: u64,
    pub step: u64,
    pub argmax: Vec<TokenId>,
}

/// Roll back every shard's KV to the committed prefix. Broadcast down the chain after
/// accept/reject. Maps to [`super::engine::TargetChain::truncate`].
#[derive(Clone, Copy, Debug)]
pub struct KvTruncate {
    pub request_id: u64,
    pub epoch: u64,
    pub keep_to_pos: u32,
}

/// A duplex link to an adjacent shard (next/prev hop), or tail→owner direct return.
///
/// TODO(luminal-glm-pipeline): implement over quinn bi-streams brokered by the router at
/// path-formation time (`PeerConnection` generalising `network::connection`). One stream
/// per (request, hop), epoch-fenced, with app-level backpressure.
pub trait PeerLink {
    fn send_hidden(&mut self, frame: &HiddenStateFrame) -> std::io::Result<()>;
    fn recv_hidden(&mut self) -> std::io::Result<HiddenStateFrame>;
}
