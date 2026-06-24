//! Pipeline-parallel sharding of the Qwen model — the node→node "cut".
//!
//! A [`Shard`] runs a contiguous global layer range on its own Luminal graph + KV cache. The
//! first shard embeds token-ids; non-first shards take a residual stream as input. The last
//! shard applies the final norm + tied lm_head and emits logits; non-last shards emit the
//! residual stream (`[seq, hidden]` F16) to hand to the next shard. [`ShardedChain`] wires N
//! shards in sequence and implements [`TargetChain`], so `run_spec_decode` drives a sharded
//! model unchanged.
//!
//! Hidden state is handed shard→shard IN-PROCESS here (as a `Vec<f32>`); the QUIC transport
//! (`transport.rs`) replaces that one hop with a wire send. The round-trip is lossless: the F16
//! boundary tensor → `get_f32` (F16→F32 exact) → `set_f32_data` (F32→F16 exact, since the data
//! originated as F16). This is the "free cut" the substrate decision rests on, on a real model.
#![allow(dead_code)] // Shard/ShardedChain are consumed by the cut test + (next) the QUIC transport.

use luminal::prelude::*;

use super::model::{KVCache, QwenConfig, QwenShard};
use super::target::{argmax, QwenRuntime};
use crate::pipeline::engine::{TargetChain, TokenId};

const EOS_TOKEN: i32 = 151645; // <|im_end|>
const STOP_TOKEN: i32 = 151643; // <|endoftext|>

/// What a shard consumes: token-ids (first shard) or a residual stream (non-first shard).
enum ShardInput {
    Tokens(Vec<i32>),
    Hidden(Vec<f32>),
}

/// One pipeline shard: a global layer range on its own Luminal graph + runtime + KV cache.
pub struct Shard<R: QwenRuntime> {
    cx: Graph,
    runtime: R,
    input: GraphTensor,   // Int token-ids ([s]) if first, else F16 hidden ([s, hidden])
    pos_ids: GraphTensor, // absolute positions
    out: GraphTensor,     // F32 logits ([s, vocab]) if last, else F16 hidden ([s, hidden])
    kv_cache: KVCache,
    cache_outputs: Vec<(GraphTensor, GraphTensor)>,
    cfg: QwenConfig,
    is_first: bool,
    is_last: bool,
    n_layers: usize,
    max_seq_len: usize,
}

impl<R: QwenRuntime + 'static> Shard<R> {
    /// Build this shard's graph with the GENERIC `s` bucket (`[2,max]` — slow multi-token kernel;
    /// see [`load_chunked`](Self::load_chunked) for the fast specialized path).
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        runtime: R,
        cfg: QwenConfig,
        range: (usize, usize),
        is_first: bool,
        is_last: bool,
        weights_path: &str,
        max_seq_len: usize,
        search_graphs: usize,
    ) -> Self {
        Self::load_chunked(
            runtime, cfg, range, is_first, is_last, weights_path, max_seq_len, search_graphs, 0,
        )
    }

    /// Like [`load`](Self::load), but `chunk_size > 0` compiles SPECIALIZED point buckets
    /// `[1,1] + [chunk_size,chunk_size]` for the fixed spec-decode chunk size (γ+1). On Metal this
    /// is ~16× faster than the generic dynamic `[2,max]` kernel (the multi-token-kernel perf bug:
    /// s=8 generic ≈ 2300ms vs specialized ≈ 138ms). Only `s ∈ {1, chunk_size}` may be fed.
    #[allow(clippy::too_many_arguments)]
    pub fn load_chunked(
        mut runtime: R,
        cfg: QwenConfig,
        range: (usize, usize),
        is_first: bool,
        is_last: bool,
        weights_path: &str,
        max_seq_len: usize,
        search_graphs: usize,
        chunk_size: usize,
    ) -> Self {
        let n_layers = range.1 - range.0;
        let mut cx = Graph::default();
        let input = if is_first {
            cx.named_tensor("input", 's').as_dtype(DType::Int)
        } else {
            cx.named_tensor("input", ('s', cfg.hidden)).as_dtype(DType::F16)
        };
        let pos_ids = cx.named_tensor("token_ids", 's').as_dtype(DType::Int);
        let kv_cache = KVCache::new(&mut cx, &cfg, max_seq_len, n_layers);
        let (out, cache_outputs) =
            QwenShard::init(&mut cx, cfg, range, is_first, is_last).forward(input, pos_ids, &kv_cache);
        let out = out.output();
        for (k, v) in &cache_outputs {
            k.output();
            v.output();
        }

        let search_s = 8usize;
        let s_buckets = if chunk_size == 0 {
            vec![DimBucket::new(1, 1), DimBucket::new(2, max_seq_len).representative(search_s)]
        } else {
            // Two POINT buckets — s=1 + s=chunk_size, both specialized (fast), no dynamic range.
            vec![DimBucket::new(1, 1), DimBucket::new(chunk_size, chunk_size)]
        };
        let rep_s = if chunk_size == 0 { search_s } else { chunk_size };
        let compile_options = CompileOptions::default().dim_buckets('s', &s_buckets).dim_buckets(
            'p',
            &[DimBucket::new(0, 0), DimBucket::new(1, max_seq_len).representative(search_s)],
        );
        cx.build_search_space::<R>(compile_options);
        runtime.load_safetensors(&cx, weights_path);

        let cache_bytes = cfg.n_kv_heads * max_seq_len * cfg.head_dim * 2; // F16
        for i in 0..n_layers {
            runtime.set_zeros(kv_cache.k_caches[i].id, cache_bytes);
            runtime.set_zeros(kv_cache.v_caches[i].id, cache_bytes);
        }

        // Representative inputs for the search (it executes candidate genomes); size to the
        // largest bucket so every bucket's profile run has a big-enough buffer.
        cx.set_dim('s', rep_s);
        cx.set_dim('p', 0);
        if is_first {
            runtime.set_i32_data(input.id, vec![1i32; rep_s]);
        } else {
            runtime.set_f32_data(input.id, vec![0.02f32; rep_s * cfg.hidden]);
        }
        runtime.set_i32_data(pos_ids.id, (0..rep_s as i32).collect());
        let search_options = CompileOptions::default().search_graph_limit(search_graphs);
        runtime = cx.search(runtime, search_options);

        for i in 0..n_layers {
            runtime.set_zeros(kv_cache.k_caches[i].id, cache_bytes);
            runtime.set_zeros(kv_cache.v_caches[i].id, cache_bytes);
        }

        Self {
            cx,
            runtime,
            input,
            pos_ids,
            out,
            kv_cache,
            cache_outputs,
            cfg,
            is_first,
            is_last,
            n_layers,
            max_seq_len,
        }
    }

    /// Run `s` positions starting at absolute `start_pos`; advance the KV; return the output
    /// (`[s, vocab]` logits if last shard, else `[s, hidden]` residual stream) flattened.
    fn run_chunk(&mut self, start_pos: u32, s: usize, input: ShardInput) -> Vec<f32> {
        self.cx.set_dim('s', s);
        self.cx.set_dim('p', start_pos as usize);
        match input {
            ShardInput::Tokens(t) => self.runtime.set_i32_data(self.input.id, t),
            ShardInput::Hidden(h) => self.runtime.set_f32_data(self.input.id, h),
        }
        self.runtime
            .set_i32_data(self.pos_ids.id, (start_pos as i32..start_pos as i32 + s as i32).collect());
        self.runtime.prepare_execute(&self.cx.dyn_map);
        self.runtime.execute(&self.cx.dyn_map);
        let out_data = self.runtime.get_f32(self.out.id);
        // Swap the updated per-layer KV (output) back into the input cache slots.
        for (i, (k_out, v_out)) in self.cache_outputs.iter().enumerate() {
            let kb = self.runtime.remove_buffer(k_out.id);
            let vb = self.runtime.remove_buffer(v_out.id);
            self.runtime.set_buffer(self.kv_cache.k_caches[i].id, kb);
            self.runtime.set_buffer(self.kv_cache.v_caches[i].id, vb);
        }
        out_data
    }

    pub fn reset(&mut self) {
        let cache_bytes = self.cfg.n_kv_heads * self.max_seq_len * self.cfg.head_dim * 2;
        for i in 0..self.n_layers {
            self.runtime.set_zeros(self.kv_cache.k_caches[i].id, cache_bytes);
            self.runtime.set_zeros(self.kv_cache.v_caches[i].id, cache_bytes);
        }
    }

    /// First shard: embed `tokens` (absolute positions from `start_pos`), run the shard, return
    /// the residual stream `[s, hidden]` flat.
    pub fn forward_tokens(&mut self, start_pos: u32, tokens: &[TokenId]) -> Vec<f32> {
        self.run_chunk(start_pos, tokens.len(), ShardInput::Tokens(tokens.to_vec()))
    }

    /// Non-first shard: consume a residual stream (`s` positions from `start_pos`), run the shard,
    /// return the residual stream (or logits `[s, vocab]` if this is the last shard) flat.
    pub fn forward_hidden(&mut self, start_pos: u32, s: usize, hidden: Vec<f32>) -> Vec<f32> {
        self.run_chunk(start_pos, s, ShardInput::Hidden(hidden))
    }

    pub fn vocab_size(&self) -> usize {
        self.cfg.vocab_size
    }

    pub fn hidden_dim(&self) -> usize {
        self.cfg.hidden
    }

    pub fn is_last(&self) -> bool {
        self.is_last
    }
}

/// A node chain: shards run in sequence, each consuming the previous shard's output. Implements
/// [`TargetChain`] so the spec-decode driver drives the sharded model unchanged. In-process
/// hidden-state hand-off (the QUIC transport replaces the shard→shard hop).
pub struct ShardedChain<R: QwenRuntime> {
    shards: Vec<Shard<R>>,
    vocab_size: usize,
}

impl<R: QwenRuntime + 'static> ShardedChain<R> {
    pub fn new(shards: Vec<Shard<R>>) -> Self {
        assert!(!shards.is_empty(), "a chain needs at least one shard");
        assert!(shards[0].is_first, "first shard must embed tokens");
        assert!(shards.last().unwrap().is_last, "last shard must emit logits");
        let vocab_size = shards[0].cfg.vocab_size;
        Self { shards, vocab_size }
    }

    /// Run `tokens` (absolute positions from `start_pos`) through every shard; return the last
    /// shard's logits as `[s, vocab]` flat.
    fn run(&mut self, start_pos: u32, tokens: &[TokenId]) -> Vec<f32> {
        let s = tokens.len();
        let last = self.shards.len() - 1;
        let mut carry: Option<ShardInput> = Some(ShardInput::Tokens(tokens.to_vec()));
        let mut logits = Vec::new();
        for (idx, shard) in self.shards.iter_mut().enumerate() {
            let input = carry.take().expect("carry is set for every shard");
            let out = shard.run_chunk(start_pos, s, input);
            if idx == last {
                logits = out;
            } else {
                carry = Some(ShardInput::Hidden(out));
            }
        }
        logits
    }

    pub fn reset(&mut self) {
        for shard in &mut self.shards {
            shard.reset();
        }
    }
}

impl<R: QwenRuntime + 'static> TargetChain for ShardedChain<R> {
    fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
        let logits = self.run(0, prompt);
        let v = self.vocab_size;
        let last = (prompt.len() - 1) * v;
        argmax(&logits[last..last + v])
    }

    fn verify(&mut self, start_pos: u32, cur: TokenId, drafts: &[TokenId]) -> Vec<TokenId> {
        let mut chunk = Vec::with_capacity(drafts.len() + 1);
        chunk.push(cur);
        chunk.extend_from_slice(drafts);
        let logits = self.run(start_pos, &chunk);
        let v = self.vocab_size;
        (0..chunk.len()).map(|j| argmax(&logits[j * v..(j + 1) * v])).collect()
    }

    fn truncate(&mut self, _keep_to_pos: u32) {
        // No-op: each shard's KV is position-indexed; the next chunk overwrites the rejected tail.
    }

    fn is_eos(&self, token: TokenId) -> bool {
        token == EOS_TOKEN || token == STOP_TOKEN
    }
}

#[cfg(all(test, feature = "metal", target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::pipeline::qwen::hf::prepare_hf_model;
    use crate::pipeline::qwen::target::QwenTarget;

    fn greedy<T: TargetChain>(t: &mut T, prompt: &[TokenId], max_new: usize) -> Vec<TokenId> {
        let mut cur = t.prefill(prompt);
        let mut out = Vec::new();
        let mut base = prompt.len() as u32;
        while !t.is_eos(cur) && out.len() < max_new {
            out.push(cur);
            cur = t.verify(base, cur, &[])[0];
            base += 1;
        }
        out
    }

    // THE CUT, on the real model: a 2-shard pipeline (hidden state round-tripped at the layer
    // boundary) must reproduce the single-process forward bit-for-bit.
    // cargo test -p dria-node --features metal qwen_cut_2shard -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_cut_2shard() {
        let cfg = QwenConfig::QWEN3_0_6B;
        let dir = prepare_hf_model("Qwen/Qwen3-0.6B").expect("prepare 0.6B");
        let w = dir.join("model_combined_bf16_v1.safetensors");
        let wp = w.to_str().unwrap();

        // Single-process baseline.
        let mut full =
            QwenTarget::load(luminal_metal::MetalRuntime::initialize(()), cfg, wp, 512, 200);

        // 2-shard chain: layers [0,k) (first) -> [k,L) (last).
        let k = cfg.layers / 2;
        let s0 = Shard::load(
            luminal_metal::MetalRuntime::initialize(()),
            cfg,
            (0, k),
            true,
            false,
            wp,
            512,
            200,
        );
        let s1 = Shard::load(
            luminal_metal::MetalRuntime::initialize(()),
            cfg,
            (k, cfg.layers),
            false,
            true,
            wp,
            512,
            200,
        );
        let mut chain = ShardedChain::new(vec![s0, s1]);

        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108];
        let max_new = 24;

        full.reset();
        let base = greedy(&mut full, &prompt, max_new);
        chain.reset();
        let sharded = greedy(&mut chain, &prompt, max_new);

        println!("\nfull (1 proc)   = {base:?}");
        println!("cut  (2 shards) = {sharded:?}");
        match (0..base.len().min(sharded.len())).find(|&i| base[i] != sharded[i]) {
            None if base.len() == sharded.len() => {
                println!("LOSSLESS CUT: 2-shard pipeline == single-process forward (0.6B, Metal)")
            }
            Some(i) => println!(
                "diverge @{i}: full={} cut={} (bf16 near-tie at the boundary?)",
                base[i], sharded[i]
            ),
            None => println!("length differ: full={} cut={}", base.len(), sharded.len()),
        }
        assert_eq!(base, sharded, "the cut must be lossless");
    }
}
