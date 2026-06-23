//! `QwenTarget` — a real [`TargetChain`] backed by the Qwen3-0.6B Luminal model.
//!
//! This is the first time a real model runs through the spec-decode driver. It mirrors the
//! upstream qwen example's run harness (build graph once → egglog search once → load weights),
//! then exposes the three ops the driver needs:
//!   - `prefill`  — run the prompt, seed the per-layer KV, return the first argmax.
//!   - `verify`   — feed `[cur]+drafts`, read the argmax at EVERY position (the example only
//!                  reads the last row), advance the KV by the chunk.
//!   - `truncate` — a NO-OP: the cache is position-indexed and `verify` is told its absolute
//!                  `start_pos`, so the next chunk overwrites the rejected tail (the
//!                  absolute-position-overwrite rollback, validated in kv_truncate_test).
//!
//! Greedy / no repetition penalty, so output is comparable to plain greedy decode.

use luminal::prelude::*;

use super::model::{KVCache, Qwen, QwenConfig};
use crate::pipeline::engine::{TargetChain, TokenId};

const EOS_TOKEN: i32 = 151645; // <|im_end|>
const STOP_TOKEN: i32 = 151643; // <|endoftext|>

/// Runtime abstraction over the Luminal backends (mirrors the qwen example's trait).
pub trait QwenRuntime: Runtime<ExecReturn = ()> {
    type Buffer;
    fn load_safetensors(&mut self, cx: &Graph, file_path: &str);
    fn set_i32_data(&mut self, id: NodeIndex, data: Vec<i32>);
    /// Feed F32 data into an input tensor; if the tensor is F16 the runtime converts (used to
    /// inject a residual-stream hidden state into a non-first pipeline shard, losslessly since
    /// the data originated as F16 from `get_f32`).
    fn set_f32_data(&mut self, id: NodeIndex, data: Vec<f32>);
    fn set_zeros(&mut self, id: NodeIndex, num_bytes: usize);
    fn remove_buffer(&mut self, id: NodeIndex) -> Self::Buffer;
    fn set_buffer(&mut self, id: NodeIndex, buffer: Self::Buffer);
    fn get_f32(&self, id: NodeIndex) -> Vec<f32>;
    fn prepare_execute(&mut self, _dyn_map: &FxHashMap<char, usize>) {}
}

#[cfg(feature = "metal")]
impl QwenRuntime for luminal_metal::MetalRuntime {
    type Buffer = luminal_metal::Buffer;
    fn load_safetensors(&mut self, cx: &Graph, file_path: &str) {
        luminal_metal::MetalRuntime::load_safetensors(self, cx, file_path);
    }
    fn set_i32_data(&mut self, id: NodeIndex, data: Vec<i32>) {
        luminal_metal::MetalRuntime::set_data(self, id, data);
    }
    fn set_f32_data(&mut self, id: NodeIndex, data: Vec<f32>) {
        luminal_metal::MetalRuntime::set_data(self, id, data);
    }
    fn set_zeros(&mut self, id: NodeIndex, num_bytes: usize) {
        luminal_metal::MetalRuntime::set_zeros(self, id, num_bytes);
    }
    fn remove_buffer(&mut self, id: NodeIndex) -> Self::Buffer {
        luminal_metal::MetalRuntime::remove_buffer(self, id)
    }
    fn set_buffer(&mut self, id: NodeIndex, buffer: Self::Buffer) {
        luminal_metal::MetalRuntime::set_buffer(self, id, buffer);
    }
    fn get_f32(&self, id: NodeIndex) -> Vec<f32> {
        luminal_metal::MetalRuntime::get_f32(self, id)
    }
}

#[cfg(feature = "cuda")]
impl QwenRuntime for luminal_cuda_lite::runtime::CudaRuntime {
    type Buffer = luminal_cuda_lite::cudarc::driver::CudaSlice<u8>;
    fn load_safetensors(&mut self, cx: &Graph, file_path: &str) {
        luminal_cuda_lite::runtime::CudaRuntime::load_safetensors(self, cx, file_path);
    }
    fn set_i32_data(&mut self, id: NodeIndex, data: Vec<i32>) {
        luminal_cuda_lite::runtime::CudaRuntime::set_data(self, id, data);
    }
    fn set_f32_data(&mut self, id: NodeIndex, data: Vec<f32>) {
        luminal_cuda_lite::runtime::CudaRuntime::set_data(self, id, data);
    }
    fn set_zeros(&mut self, id: NodeIndex, num_bytes: usize) {
        luminal_cuda_lite::runtime::CudaRuntime::set_zeros(self, id, num_bytes);
    }
    fn remove_buffer(&mut self, id: NodeIndex) -> Self::Buffer {
        luminal_cuda_lite::runtime::CudaRuntime::remove_buffer(self, id)
    }
    fn set_buffer(&mut self, id: NodeIndex, buffer: Self::Buffer) {
        luminal_cuda_lite::runtime::CudaRuntime::set_buffer(self, id, buffer);
    }
    fn get_f32(&self, id: NodeIndex) -> Vec<f32> {
        luminal_cuda_lite::runtime::CudaRuntime::get_f32(self, id)
    }
}

pub(crate) fn argmax(row: &[f32]) -> TokenId {
    let mut best = 0usize;
    let mut best_v = f32::NEG_INFINITY;
    for (i, &v) in row.iter().enumerate() {
        if v > best_v {
            best_v = v;
            best = i;
        }
    }
    best as TokenId
}

pub struct QwenTarget<R: QwenRuntime> {
    cx: Graph,
    runtime: R,
    input: GraphTensor,     // token ids to embed
    pos_ids: GraphTensor,   // absolute positions (RoPE + cache index)
    logits: GraphTensor,    // [s, VOCAB] for every fed position
    kv_cache: KVCache,
    cache_outputs: Vec<(GraphTensor, GraphTensor)>,
    cfg: QwenConfig,
    max_seq_len: usize,
}

impl<R: QwenRuntime + 'static> QwenTarget<R> {
    /// Build the graph, run the egglog search, and load the bf16 weights. `weights_path` is
    /// the combined safetensors produced by `super::hf::prepare_hf_model`.
    pub fn load(
        mut runtime: R,
        cfg: QwenConfig,
        weights_path: &str,
        max_seq_len: usize,
        search_graphs: usize,
    ) -> Self {
        let mut cx = Graph::default();
        let input = cx.named_tensor("input", 's').as_dtype(DType::Int);
        let pos_ids = cx.named_tensor("token_ids", 's').as_dtype(DType::Int);
        let kv_cache = KVCache::new(&mut cx, &cfg, max_seq_len, cfg.layers);
        let (logits, cache_outputs) = Qwen::init(&mut cx, cfg).forward(input, pos_ids, &kv_cache);
        let logits = logits.output();
        for (k, v) in &cache_outputs {
            k.output();
            v.output();
        }

        // Compile once across the seq-len ('s') and past-len ('p') ranges we will use.
        let search_s = 8usize;
        let compile_options = CompileOptions::default()
            .dim_buckets(
                's',
                &[DimBucket::new(1, 1), DimBucket::new(2, max_seq_len).representative(search_s)],
            )
            .dim_buckets(
                'p',
                &[DimBucket::new(0, 0), DimBucket::new(1, max_seq_len).representative(search_s)],
            );
        cx.build_search_space::<R>(compile_options);
        runtime.load_safetensors(&cx, weights_path);

        let cache_bytes = cfg.n_kv_heads * max_seq_len * cfg.head_dim * 2; // F16
        for i in 0..cfg.layers {
            runtime.set_zeros(kv_cache.k_caches[i].id, cache_bytes);
            runtime.set_zeros(kv_cache.v_caches[i].id, cache_bytes);
        }

        // Search needs representative inputs present (it executes candidate genomes).
        cx.set_dim('s', search_s);
        cx.set_dim('p', 0);
        runtime.set_i32_data(input.id, vec![1; search_s]);
        runtime.set_i32_data(pos_ids.id, (0..search_s as i32).collect());
        let search_options = CompileOptions::default().search_graph_limit(search_graphs);
        runtime = cx.search(runtime, search_options);

        for i in 0..cfg.layers {
            runtime.set_zeros(kv_cache.k_caches[i].id, cache_bytes);
            runtime.set_zeros(kv_cache.v_caches[i].id, cache_bytes);
        }

        Self { cx, runtime, input, pos_ids, logits, kv_cache, cache_outputs, cfg, max_seq_len }
    }

    /// Run a chunk of `tokens` starting at absolute position `start_pos`; advance the KV; return
    /// the per-position logits as `[s, VOCAB]` flattened.
    fn run_chunk(&mut self, start_pos: u32, tokens: &[TokenId]) -> Vec<f32> {
        let s = tokens.len();
        self.cx.set_dim('s', s);
        self.cx.set_dim('p', start_pos as usize);
        self.runtime.set_i32_data(self.input.id, tokens.to_vec());
        self.runtime
            .set_i32_data(self.pos_ids.id, (start_pos as i32..start_pos as i32 + s as i32).collect());
        self.runtime.prepare_execute(&self.cx.dyn_map);
        self.runtime.execute(&self.cx.dyn_map);
        let logits_data = self.runtime.get_f32(self.logits.id);
        // swap the updated per-layer KV (output) back into the input cache slots.
        for (i, (k_out, v_out)) in self.cache_outputs.iter().enumerate() {
            let kb = self.runtime.remove_buffer(k_out.id);
            let vb = self.runtime.remove_buffer(v_out.id);
            self.runtime.set_buffer(self.kv_cache.k_caches[i].id, kb);
            self.runtime.set_buffer(self.kv_cache.v_caches[i].id, vb);
        }
        logits_data
    }

    /// Zero the per-layer KV cache (e.g. between two independent generations).
    pub fn reset(&mut self) {
        let cache_bytes = self.cfg.n_kv_heads * self.max_seq_len * self.cfg.head_dim * 2;
        for i in 0..self.cfg.layers {
            self.runtime.set_zeros(self.kv_cache.k_caches[i].id, cache_bytes);
            self.runtime.set_zeros(self.kv_cache.v_caches[i].id, cache_bytes);
        }
    }
}

impl<R: QwenRuntime + 'static> TargetChain for QwenTarget<R> {
    fn prefill(&mut self, prompt: &[TokenId]) -> TokenId {
        let logits = self.run_chunk(0, prompt);
        let v = self.cfg.vocab_size;
        let last = (prompt.len() - 1) * v;
        argmax(&logits[last..last + v])
    }

    fn verify(&mut self, start_pos: u32, cur: TokenId, drafts: &[TokenId]) -> Vec<TokenId> {
        let mut chunk = Vec::with_capacity(drafts.len() + 1);
        chunk.push(cur);
        chunk.extend_from_slice(drafts);
        let logits = self.run_chunk(start_pos, &chunk);
        let v = self.cfg.vocab_size;
        (0..chunk.len())
            .map(|j| argmax(&logits[j * v..(j + 1) * v]))
            .collect()
    }

    fn truncate(&mut self, _keep_to_pos: u32) {
        // No-op: the cache is position-indexed and `verify` is given the absolute start_pos,
        // so the next chunk overwrites the rejected tail (absolute-position rollback).
    }

    fn is_eos(&self, token: TokenId) -> bool {
        token == EOS_TOKEN || token == STOP_TOKEN
    }
}

#[cfg(all(test, feature = "metal", target_vendor = "apple"))]
mod tests {
    use super::*;
    use crate::pipeline::engine::DraftEngine;
    use crate::pipeline::specdecode::{run_spec_decode, SpecConfig};

    /// Prompt-lookup n-gram drafter (no second model) — exercises accept AND reject.
    struct NgramDraft;
    impl DraftEngine for NgramDraft {
        fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            let n = context.len();
            if n >= 2 {
                let (a, b) = (context[n - 2], context[n - 1]);
                for i in (0..n - 2).rev() {
                    if context[i] == a && context[i + 1] == b {
                        let start = i + 2;
                        let mut d: Vec<TokenId> =
                            (0..gamma).filter_map(|k| context.get(start + k).copied()).collect();
                        if !d.is_empty() {
                            while d.len() < gamma {
                                d.push(*d.last().unwrap());
                            }
                            return d;
                        }
                    }
                }
            }
            vec![*context.last().unwrap_or(&0); gamma]
        }
    }

    /// Plain incremental greedy decode via the verify primitive (empty draft → 1 token/step).
    fn greedy_baseline(t: &mut impl TargetChain, prompt: &[TokenId], max_new: usize) -> Vec<TokenId> {
        let mut cur = t.prefill(prompt);
        let mut out = Vec::new();
        let mut base = prompt.len() as u32;
        loop {
            if t.is_eos(cur) || out.len() >= max_new {
                break;
            }
            out.push(cur);
            cur = t.verify(base, cur, &[])[0];
            base += 1;
        }
        out
    }

    // cargo test -p dria-node --features metal qwen_target_spec_decode -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_target_spec_decode() {
        let model_dir = crate::pipeline::qwen::hf::prepare_hf_model("Qwen/Qwen3-0.6B")
            .expect("download/prepare Qwen3-0.6B");
        let weights = model_dir.join("model_combined_bf16_v1.safetensors");
        let rt = luminal_metal::MetalRuntime::initialize(());
        let mut target = QwenTarget::load(rt, QwenConfig::QWEN3_0_6B, weights.to_str().unwrap(), 512, 200);

        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108]; // raw token ids (no tokenizer)
        let max_new = 32;
        let cfg = SpecConfig { gamma: 4, max_new };

        target.reset();
        let baseline = greedy_baseline(&mut target, &prompt, max_new);

        target.reset();
        let mut draft = NgramDraft;
        let (spec, stats) = run_spec_decode(&mut draft, &mut target, &prompt, &cfg);

        println!("\nbaseline = {baseline:?}");
        println!("spec     = {spec:?}");
        println!("toks/traversal = {:.2}", stats.toks_per_traversal());
        match (0..baseline.len().min(spec.len())).find(|&i| baseline[i] != spec[i]) {
            None if baseline.len() == spec.len() => {
                println!("LOSSLESS: spec-decode == greedy baseline on Qwen3-0.6B (Luminal/Metal)")
            }
            Some(i) => println!(
                "DIVERGE @{i}: baseline={} spec={} (likely a bf16 near-tie — cf. M1 substrate finding)",
                baseline[i], spec[i]
            ),
            None => println!("length differ: baseline={} spec={}", baseline.len(), spec.len()),
        }
        assert!(!baseline.is_empty(), "model must generate something");
    }

    /// A real draft model: greedily generate `gamma` tokens from a smaller QwenTarget.
    /// Re-prefills the whole context on each call (O(context) — fine for a g measurement;
    /// the production draft keeps an incremental KV).
    struct QwenDraft<'a, R: QwenRuntime + 'static> {
        model: &'a mut QwenTarget<R>,
    }
    impl<R: QwenRuntime + 'static> DraftEngine for QwenDraft<'_, R> {
        fn draft(&mut self, context: &[TokenId], gamma: usize) -> Vec<TokenId> {
            self.model.reset();
            let mut cur = self.model.prefill(context);
            let mut out = Vec::with_capacity(gamma);
            out.push(cur);
            let mut base = context.len() as u32;
            for _ in 1..gamma {
                cur = self.model.verify(base, cur, &[])[0];
                out.push(cur);
                base += 1;
            }
            out
        }
    }

    // THE REAL ACCEPTANCE NUMBER: Qwen3-0.6B drafts Qwen3-4B, greedy, on Luminal/Metal.
    // toks/traversal = E[accepted]+1 is the speedup proxy the throughput thesis hinges on.
    // cargo test -p dria-node --features metal qwen_real_g_4b -- --ignored --nocapture
    #[test]
    #[ignore]
    fn qwen_real_g_4b() {
        let dir4b = crate::pipeline::qwen::hf::prepare_hf_model("Qwen/Qwen3-4B").expect("prepare 4B");
        let dir06b =
            crate::pipeline::qwen::hf::prepare_hf_model("Qwen/Qwen3-0.6B").expect("prepare 0.6B");
        let w4b = dir4b.join("model_combined_bf16_v1.safetensors");
        let w06b = dir06b.join("model_combined_bf16_v1.safetensors");

        let mut target = QwenTarget::load(
            luminal_metal::MetalRuntime::initialize(()),
            QwenConfig::QWEN3_4B,
            w4b.to_str().unwrap(),
            512,
            200,
        );
        let mut draft_model = QwenTarget::load(
            luminal_metal::MetalRuntime::initialize(()),
            QwenConfig::QWEN3_0_6B,
            w06b.to_str().unwrap(),
            512,
            200,
        );

        let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108]; // raw token ids
        let max_new = 48;
        let spec_cfg = SpecConfig { gamma: 8, max_new };

        target.reset();
        let baseline = greedy_baseline(&mut target, &prompt, max_new);

        target.reset();
        draft_model.reset();
        let mut draft = QwenDraft { model: &mut draft_model };
        let (spec, stats) = run_spec_decode(&mut draft, &mut target, &prompt, &spec_cfg);

        let tpt = stats.toks_per_traversal();
        println!("\n4B baseline           = {baseline:?}");
        println!("spec (0.6B drafts 4B) = {spec:?}");
        println!(
            "toks/traversal = {tpt:.2}  (= E[accepted]+1, gamma={})  => mean accepted/round = {:.2}",
            spec_cfg.gamma,
            tpt - 1.0
        );
        match (0..baseline.len().min(spec.len())).find(|&i| baseline[i] != spec[i]) {
            None if baseline.len() == spec.len() => println!("LOSSLESS vs greedy-4B"),
            Some(i) => println!(
                "diverge @{i}: baseline={} spec={} (bf16 near-tie? cf. M1)",
                baseline[i], spec[i]
            ),
            None => println!("length differ: baseline={} spec={}", baseline.len(), spec.len()),
        }
        assert!(!spec.is_empty(), "spec-decode must produce output");
    }
}
