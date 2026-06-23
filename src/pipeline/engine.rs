//! Substrate-agnostic engine traits for pipeline-parallel GLM-5.2 serving.
//!
//! The whole point of these traits is that the speculative-decode **driver**
//! ([`super::specdecode`]) is written purely against them — so its accept/reject +
//! KV-rollback logic is unit-testable with synthetic engines, with zero dependency on
//! Luminal, a GPU, or a real model. The production implementations are:
//!   - [`DraftEngine`]  → a small Luminal model on node-0 (e.g. GLM-4-9B), off the WAN.
//!   - [`TargetChain`]  → the GLM-5.2 layers sharded across the node chain, fanned out
//!                        over QUIC; to the driver it is just a black box that verifies a
//!                        chunk and can truncate its (per-shard) KV.
//!
//! See `docs/pipeline-specdecode-design.md` and `docs/substrate-decision.md`.

/// A vocabulary token id. Matches llama/Luminal/HF conventions (i32).
pub type TokenId = i32;

/// The local draft model. Runs entirely on node-0 (the path owner), off the WAN, and
/// proposes the next `gamma` tokens greedily given the committed context.
pub trait DraftEngine {
    /// Greedily draft `gamma` tokens continuing `context` (the committed token ids,
    /// i.e. `prompt ++ generated ++ [pending]`). Must return exactly `gamma` tokens.
    fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId>;
}

/// The sharded target model (GLM-5.2), exposed to the driver as a black box.
///
/// In production a single call here fans the hidden state out across the shard chain over
/// QUIC and reads the result back; here it is a trait so the driver stays substrate- and
/// transport-agnostic.
pub trait TargetChain {
    /// Prefill `prompt` into the chain's (per-shard) KV — leaving each shard's cache
    /// holding positions `[0, prompt.len())` — and return the target's greedy argmax for
    /// the first generated position (the `cur` token pending at position `prompt.len()`).
    fn prefill(&mut self, prompt: &[TokenId]) -> TokenId;

    /// Verify the candidate chunk `[cur] + drafts`, whose first token (`cur`) sits at
    /// absolute KV position `start_pos`. Advances each shard's KV by the full chunk
    /// (`drafts.len() + 1` positions) and returns the target's greedy argmax at **each**
    /// of those positions — i.e. `out[j]` is what the target would produce after the
    /// `j`-th fed token. The returned vector has length `drafts.len() + 1`.
    ///
    /// The driver compares `out[j]` against `drafts[j]` to find the accepted prefix, then
    /// calls [`TargetChain::truncate`] to discard the rejected speculative tail.
    fn verify(&mut self, start_pos: u32, cur: TokenId, drafts: &[TokenId]) -> Vec<TokenId>;

    /// Roll back every shard's KV to keep only positions `[0, keep_to_pos)` — the
    /// `KvTruncate` primitive. This is trivial on Luminal (the host owns the cache, so a
    /// truncate is dropping page-table slots), and is the exact operation llama.cpp's
    /// `seq_rm` *refused* on SWA/hybrid caches — the M1 finding that drove the substrate
    /// decision (see `docs/substrate-decision.md`).
    fn truncate(&mut self, keep_to_pos: u32);

    /// Whether `token` terminates generation (EOS/EOG).
    fn is_eos(&self, token: TokenId) -> bool;
}
