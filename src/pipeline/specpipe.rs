//! Async depth-N **pipelined** speculative-decode driver — shard's `coordinate_pipe` in Rust.
//!
//! The synchronous driver ([`super::specdecode::run_spec_decode`]) pays one full chain
//! traversal per committed chunk (the ~3 tok/s regime over WAN). This driver keeps `depth`
//! verify chunks **in flight at once** over a direct-return ring, speculatively chaining
//! each chunk on the previous chunk's *draft* output — so throughput approaches the ring's
//! per-stage throughput, not its full latency (shard measured 2.94 → 16.6 tok/s). On a
//! divergence, every still-in-flight chunk is stale: they're drained-and-discarded and
//! drafting re-seeds from the corrected prefix. Greedy id-equality keeps the committed
//! output **bit-identical to the synchronous driver** (and to plain greedy decode).
//!
//! See `docs/shard-adoption-brief.md` (technique #1) and `docs/pipeline-specdecode-design.md`.

use std::collections::VecDeque;

use super::engine::{DraftEngine, TokenId};
use super::specdecode::{SpecConfig, SpecStats};

/// A target chain that supports depth-N pipelining: many verify chunks in flight at once,
/// completing in FIFO (submission) order — the "direct-return ring" (tail → owner, 1 hop).
pub trait PipelinedChain {
    /// Prefill `prompt`; return the first pending token (the chain's argmax after the prompt).
    fn prefill(&mut self, prompt: &[TokenId]) -> TokenId;
    /// Submit a verify chunk `[cur] + drafts` whose first token sits at absolute KV position
    /// `start_pos`. Non-blocking — the chunk enters the pipeline.
    fn submit(&mut self, start_pos: u32, cur: TokenId, drafts: &[TokenId]);
    /// Block for the next completed chunk's per-position argmaxes (FIFO; length `drafts.len()+1`).
    fn poll(&mut self) -> Vec<TokenId>;
    /// Roll back KV to keep `[0, keep_to_pos)` after a divergence flush.
    fn truncate(&mut self, keep_to_pos: u32);
    /// Whether `token` terminates generation.
    fn is_eos(&self, token: TokenId) -> bool;
}

/// Run depth-N pipelined greedy speculative decode. `depth` chunks are kept in flight.
/// Output is identical to [`super::specdecode::run_spec_decode`] (and to plain greedy).
pub fn run_spec_decode_pipelined<D: DraftEngine, C: PipelinedChain>(
    draft: &mut D,
    chain: &mut C,
    prompt: &[TokenId],
    cfg: &SpecConfig,
    depth: usize,
) -> (Vec<TokenId>, SpecStats) {
    let gamma = cfg.gamma.max(1);
    let depth = depth.max(1);
    let prompt_len = prompt.len() as u32;

    let cur0 = chain.prefill(prompt);
    let mut out: Vec<TokenId> = Vec::new();
    let mut done = chain.is_eos(cur0) || cfg.max_new == 0;
    if !done {
        out.push(cur0); // out is seeded with the first pending token (shard: `out = [cur]`)
    }

    // `dprefix` is the speculative draft chain; its last element is the next chunk's `cur`.
    let mut dprefix: Vec<TokenId> = prompt.to_vec();
    dprefix.push(cur0);
    // Consecutive chunks overlap by 1 token (their shared `cur`), so send_pos advances by
    // `gamma`, not `gamma + 1` (the re-fed `cur` lands on the previous chunk's last position).
    let mut send_pos = prompt_len;

    let mut inflight: VecDeque<Vec<TokenId>> = VecDeque::new(); // drafts per in-flight chunk
    let mut discard = 0usize; // stale post-divergence results still to drain
    let mut traversals = 0usize;

    loop {
        // FILL: keep `depth` chunks marching, chaining speculatively on `dprefix`.
        while inflight.len() < depth && !done {
            let cur = *dprefix.last().unwrap();
            if chain.is_eos(cur) {
                done = true;
                break;
            }
            let ds = draft.draft(&dprefix, gamma);
            chain.submit(send_pos, cur, &ds);
            dprefix.extend_from_slice(&ds);
            inflight.push_back(ds);
            send_pos += gamma as u32;
        }
        if inflight.is_empty() {
            break;
        }

        // READ one completed chunk (FIFO).
        let r = chain.poll();
        let ds = inflight.pop_front().unwrap();
        traversals += 1;
        if discard > 0 {
            discard -= 1; // stale (post-divergence or post-terminal) → skip
            continue;
        }

        // Accept the longest prefix matching the target's argmax.
        let mut n = 0usize;
        while n < ds.len() && n < r.len() && ds[n] == r[n] {
            n += 1;
        }
        let diverged = n != ds.len();
        let committed: Vec<TokenId> = if diverged {
            let mut c = ds[..n].to_vec();
            c.push(r[n]); // accepted prefix + correction
            c
        } else {
            ds // all accepted
        };

        for t in committed {
            if chain.is_eos(t) {
                done = true;
                break;
            }
            out.push(t);
            if out.len() >= cfg.max_new {
                done = true;
                break;
            }
        }

        if done {
            // Terminal: stop filling; drain the chunks still in flight.
            discard = inflight.len();
        } else if diverged {
            // Divergence: every still-in-flight chunk was drafted on a now-wrong prefix.
            discard = inflight.len();
            // `out` includes the seed `cur0`, so the committed KV boundary is
            // prompt_len + out.len() - 1 (shard's `pos`). The next chunk re-feeds the
            // correction at this position, overwriting the stale tail.
            let keep = prompt_len + out.len() as u32 - 1;
            chain.truncate(keep);
            dprefix = prompt.to_vec();
            dprefix.extend_from_slice(&out);
            send_pos = keep;
        }
    }

    let committed = out.len();
    (out, SpecStats { committed, traversals })
}

#[cfg(test)]
mod tests {
    use super::super::engine::TargetChain;
    use super::*;

    /// Pipelined synthetic chain: its greedy continuation of any prompt is a fixed `truth`.
    /// FIFO queue of submitted chunks' `(start_pos, n_drafts)`; `poll` computes argmaxes
    /// purely from `start_pos` (stateless ⇒ `truncate` is a no-op). The driver must
    /// reproduce `truth` exactly, at every `depth`, with any draft.
    struct SyntheticPipe {
        truth: Vec<TokenId>,
        prompt_len: u32,
        eos: TokenId,
        q: VecDeque<(u32, usize)>,
    }
    impl SyntheticPipe {
        fn new(truth: Vec<TokenId>, eos: TokenId) -> Self {
            Self { truth, prompt_len: 0, eos, q: VecDeque::new() }
        }
    }
    impl PipelinedChain for SyntheticPipe {
        fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
            self.prompt_len = prompt.len() as u32;
            *self.truth.first().unwrap_or(&self.eos)
        }
        fn submit(&mut self, start_pos: u32, _cur: TokenId, drafts: &[TokenId]) {
            self.q.push_back((start_pos, drafts.len()));
        }
        fn poll(&mut self) -> Vec<TokenId> {
            let (start_pos, len) = self.q.pop_front().expect("poll with no in-flight chunk");
            let l = (start_pos - self.prompt_len) as usize; // gen index of `cur`
            (0..=len).map(|j| *self.truth.get(l + 1 + j).unwrap_or(&self.eos)).collect()
        }
        fn truncate(&mut self, _keep_to_pos: u32) {}
        fn is_eos(&self, token: TokenId) -> bool {
            token == self.eos
        }
    }

    struct PerfectDraft {
        truth: Vec<TokenId>,
        prompt_len: usize,
    }
    impl DraftEngine for PerfectDraft {
        fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            let l = context.len() - self.prompt_len;
            (0..gamma).map(|j| *self.truth.get(l + j).unwrap_or(&-1)).collect()
        }
    }
    struct GarbageDraft;
    impl DraftEngine for GarbageDraft {
        fn draft(&mut self, _context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            vec![i32::MIN; gamma]
        }
    }

    #[test]
    fn pipelined_is_lossless_across_depths_and_draft_quality() {
        let truth: Vec<TokenId> = (1..=80).collect();
        let eos = 9999;
        let prompt = vec![5, 6, 7];
        let cfg = SpecConfig { gamma: 4, max_new: 50 };
        let expected: Vec<TokenId> = truth[..cfg.max_new].to_vec();

        for depth in [1usize, 2, 4, 8] {
            // Perfect draft: pipeline stays full, no flushes.
            let mut c = SyntheticPipe::new(truth.clone(), eos);
            let mut d = PerfectDraft { truth: truth.clone(), prompt_len: prompt.len() };
            let (out_p, _) = run_spec_decode_pipelined(&mut d, &mut c, &prompt, &cfg, depth);
            assert_eq!(out_p, expected, "perfect draft, depth={depth}");

            // Garbage draft: every chunk diverges ⇒ exercises the flush/discard path hard.
            let mut c2 = SyntheticPipe::new(truth.clone(), eos);
            let mut g = GarbageDraft;
            let (out_g, _) = run_spec_decode_pipelined(&mut g, &mut c2, &prompt, &cfg, depth);
            assert_eq!(out_g, expected, "garbage draft, depth={depth}");
        }
    }

    #[test]
    fn pipelined_matches_synchronous_driver() {
        // The pipelined and synchronous drivers must produce identical greedy output.
        let truth: Vec<TokenId> = (100..200).collect();
        let eos = -7;
        let prompt = vec![1, 2];
        let cfg = SpecConfig { gamma: 6, max_new: 60 };

        // synchronous reference (reuse the sync driver's synthetic target via a thin adapter)
        struct SyncTarget {
            truth: Vec<TokenId>,
            prompt_len: u32,
            eos: TokenId,
        }
        impl TargetChain for SyncTarget {
            fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
                self.prompt_len = prompt.len() as u32;
                *self.truth.first().unwrap_or(&self.eos)
            }
            fn verify(&mut self, start_pos: u32, _cur: TokenId, drafts: &[TokenId]) -> Vec<TokenId> {
                let l = (start_pos - self.prompt_len) as usize;
                (0..=drafts.len()).map(|j| *self.truth.get(l + 1 + j).unwrap_or(&self.eos)).collect()
            }
            fn truncate(&mut self, _: u32) {}
            fn is_eos(&self, t: TokenId) -> bool {
                t == self.eos
            }
        }

        let mut st = SyncTarget { truth: truth.clone(), prompt_len: 0, eos };
        let mut sd = PerfectDraft { truth: truth.clone(), prompt_len: prompt.len() };
        let (sync_out, _) = super::super::specdecode::run_spec_decode(&mut sd, &mut st, &prompt, &cfg);

        let mut pc = SyntheticPipe::new(truth.clone(), eos);
        let mut pd = PerfectDraft { truth: truth.clone(), prompt_len: prompt.len() };
        let (pipe_out, _) = run_spec_decode_pipelined(&mut pd, &mut pc, &prompt, &cfg, 4);

        assert_eq!(sync_out, pipe_out, "pipelined output must equal synchronous output");
    }
}
