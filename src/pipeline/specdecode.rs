//! Greedy speculative-decode driver — the orchestration core of the GLM-5.2 pipeline.
//!
//! Substrate-agnostic: it drives a [`DraftEngine`] (node-0 local) against a
//! [`TargetChain`] (the sharded target) over the traits in [`super::engine`], so the
//! accept/reject + KV-rollback logic here is identical whether the chain is Luminal on
//! CUDA, a synthetic test, or a single process. Each round:
//!   1. draft `gamma` tokens locally,
//!   2. verify `[cur] + drafts` in one chain traversal,
//!   3. accept the longest prefix matching the target's greedy argmax, take the correction,
//!   4. roll back the speculative KV tail (`KvTruncate`).
//!
//! Output is **bit-identical to plain greedy decode of the target** (greedy token-id
//! equality), regardless of draft quality — the losslessness property validated in M1
//! (`src/inference/engine.rs::test_spec_decode_characterization_qwen`) where the substrate
//! is deterministic. This is the same loop, lifted off llama.cpp and onto the trait
//! boundary so it serves the Luminal pipeline.

use super::engine::{DraftEngine, TargetChain, TokenId};

/// Spec-decode knobs.
pub struct SpecConfig {
    /// Draft length per chunk. Invest in acceptance rate, not large gamma: `gamma ∈ [8,16]`.
    pub gamma: usize,
    /// Generation cap (tokens).
    pub max_new: usize,
}

/// Per-generation telemetry.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpecStats {
    pub committed: usize,
    pub traversals: usize,
}

impl SpecStats {
    /// Committed tokens per chain traversal — the speedup proxy (`≈ E[accepted] + 1`).
    pub fn toks_per_traversal(&self) -> f64 {
        self.committed as f64 / self.traversals.max(1) as f64
    }
}

/// Run greedy speculative decode. `prompt` is the committed prefix; the driver prefills it
/// into the chain, then generates up to `cfg.max_new` tokens. Returns the generated tokens
/// (excluding the prompt) plus telemetry.
pub fn run_spec_decode<D: DraftEngine, T: TargetChain>(
    draft: &mut D,
    target: &mut T,
    prompt: &[TokenId],
    cfg: &SpecConfig,
) -> (Vec<TokenId>, SpecStats) {
    let gamma = cfg.gamma.max(1);
    let mut out: Vec<TokenId> = Vec::new();
    let mut traversals = 0usize;

    // Prefill the prompt → chain KV holds [0, prompt.len()); `cur` is the pending token.
    let mut cur = target.prefill(prompt);
    let mut base = prompt.len() as u32; // absolute KV position where `cur` is fed

    'gen: loop {
        if target.is_eos(cur) || out.len() >= cfg.max_new {
            break;
        }

        // Draft gamma tokens continuing [prompt ++ out ++ cur] (all local to node-0).
        let mut context = Vec::with_capacity(prompt.len() + out.len() + 1);
        context.extend_from_slice(prompt);
        context.extend_from_slice(&out);
        context.push(cur);
        let drafts = draft.draft(&context, gamma);

        // One chain traversal: verify [cur] + drafts at positions [base, base+drafts.len()].
        // `r[j]` = target greedy argmax after the j-th fed token (length drafts.len()+1).
        let r = target.verify(base, cur, &drafts);
        debug_assert_eq!(r.len(), drafts.len() + 1, "verify must return drafts.len()+1 argmaxes");
        traversals += 1;

        // Accept the longest prefix where the draft matched the target's argmax.
        let mut n = 0usize;
        while n < drafts.len() && n < r.len() && drafts[n] == r[n] {
            n += 1;
        }
        // Correction/bonus: target's own token at the first mismatch (or after the last
        // accepted draft if all accepted). Becomes next round's `cur`.
        let correction = r[n];

        // Commit `cur` + the `n` accepted drafts, stopping at EOS to match plain greedy.
        for t in std::iter::once(cur).chain(drafts[..n].iter().copied()) {
            if target.is_eos(t) {
                break 'gen;
            }
            out.push(t);
            if out.len() >= cfg.max_new {
                break 'gen;
            }
        }

        // Roll back the speculative KV tail: keep [0, base + n + 1) (cur + accepted drafts).
        // Skip when everything was accepted (nothing speculative to drop).
        if n < drafts.len() {
            target.truncate(base + n as u32 + 1);
        }

        cur = correction;
        base += n as u32 + 1;
    }

    let committed = out.len();
    (out, SpecStats { committed, traversals })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic target whose greedy continuation of any prompt is a fixed `truth`
    /// sequence. Because the greedy output is well-defined by position, the driver must
    /// reproduce `truth` exactly — regardless of how good or bad the draft is. (Stateless:
    /// the absolute `start_pos` encodes how many tokens are committed, so no KV bookkeeping
    /// is needed; `truncate` is therefore a no-op here.)
    struct SyntheticTarget {
        truth: Vec<TokenId>,
        prompt_len: u32,
        eos: TokenId,
    }

    impl TargetChain for SyntheticTarget {
        fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
            self.prompt_len = prompt.len() as u32;
            *self.truth.first().unwrap_or(&self.eos)
        }
        fn verify(&mut self, start_pos: u32, _cur: TokenId, drafts: &[TokenId]) -> Vec<TokenId> {
            // gen tokens committed so far = start_pos - prompt_len; `cur` is truth[l].
            let l = (start_pos - self.prompt_len) as usize;
            (0..=drafts.len())
                .map(|j| *self.truth.get(l + 1 + j).unwrap_or(&self.eos))
                .collect()
        }
        fn truncate(&mut self, _keep_to_pos: u32) {}
        fn is_eos(&self, token: TokenId) -> bool {
            token == self.eos
        }
    }

    /// Always proposes the true next tokens → maximal acceptance.
    struct PerfectDraft {
        truth: Vec<TokenId>,
        prompt_len: usize,
    }
    impl DraftEngine for PerfectDraft {
        fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            // context = prompt ++ out ++ [cur]; next truth index to draft = context.len()-prompt_len.
            let l = context.len() - self.prompt_len;
            (0..gamma).map(|j| *self.truth.get(l + j).unwrap_or(&-1)).collect()
        }
    }

    /// Always proposes a token that can't be in `truth` → zero acceptance.
    struct GarbageDraft;
    impl DraftEngine for GarbageDraft {
        fn draft(&mut self, _context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            vec![i32::MIN; gamma]
        }
    }

    #[test]
    fn spec_decode_is_lossless_regardless_of_draft_quality() {
        let truth: Vec<TokenId> = (1..=60).collect(); // the target's greedy ground truth
        let eos = 9999;
        let prompt = vec![100, 101, 102];
        let cfg = SpecConfig { gamma: 4, max_new: 40 };
        let expected: Vec<TokenId> = truth[..cfg.max_new].to_vec();

        // Perfect draft → full acceptance, but output is exactly greedy truth.
        let mut t = SyntheticTarget { truth: truth.clone(), prompt_len: 0, eos };
        let mut d = PerfectDraft { truth: truth.clone(), prompt_len: prompt.len() };
        let (out_perfect, sp) = run_spec_decode(&mut d, &mut t, &prompt, &cfg);

        // Garbage draft → zero acceptance, output STILL exactly greedy truth (losslessness).
        let mut t2 = SyntheticTarget { truth: truth.clone(), prompt_len: 0, eos };
        let mut g = GarbageDraft;
        let (out_garbage, sg) = run_spec_decode(&mut g, &mut t2, &prompt, &cfg);

        assert_eq!(out_perfect, expected, "perfect-draft output must equal greedy truth");
        assert_eq!(out_garbage, expected, "garbage-draft output must STILL equal greedy truth");

        // Acceptance changes SPEED, never correctness: perfect commits gamma+1/traversal,
        // garbage commits exactly 1/traversal.
        assert_eq!(sg.toks_per_traversal(), 1.0, "garbage draft = 1 token/traversal");
        assert!(
            sp.toks_per_traversal() > sg.toks_per_traversal(),
            "perfect draft must be faster ({} vs {})",
            sp.toks_per_traversal(),
            sg.toks_per_traversal()
        );
        assert!((sp.toks_per_traversal() - (cfg.gamma as f64 + 1.0)).abs() < 1e-9);
    }

    #[test]
    fn spec_decode_stops_at_eos() {
        // truth ends with EOS at index 5 → generation must stop there, 5 tokens out.
        let eos = 9999;
        let truth: Vec<TokenId> = vec![1, 2, 3, 4, 5, eos];
        let prompt = vec![7, 8];
        let cfg = SpecConfig { gamma: 4, max_new: 100 };

        let mut t = SyntheticTarget { truth: truth.clone(), prompt_len: 0, eos };
        let mut d = PerfectDraft { truth: truth.clone(), prompt_len: prompt.len() };
        let (out, _) = run_spec_decode(&mut d, &mut t, &prompt, &cfg);

        assert_eq!(out, vec![1, 2, 3, 4, 5], "must stop before EOS, not emit it");
    }
}
