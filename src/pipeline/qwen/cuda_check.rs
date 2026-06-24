//! CUDA substrate de-risk — run on a rented NVIDIA GPU (not Metal). Answers the three questions
//! that gate the production (luminal_cuda_lite / NVFP4) lane before committing to Blackwell:
//!   (1) Does Luminal CUDA build + run at all? (it compiles kernels at runtime via NVRTC — needs
//!       the CUDA toolkit, not just the driver). Loading + one forward proves the path.
//!   (2) Is the forward CORRECT on CUDA? (greedy a few tokens, eyeball them).
//!   (3) Does the multi-token kernel CLIFF exist on CUDA? On Metal the generic `[2,max]` bucket is
//!       ~20× slower than `[1,1]` for s≥2 (the bug `load_chunked` works around). If CUDA has no
//!       cliff, the specialized bucket is a Metal-only workaround and production needs nothing.
//!
//! This module only compiles under `--features cuda` (see mod.rs gate).
//!   cargo test --features cuda cuda_substrate_check -- --ignored --nocapture
//!
//! NOTE: model.rs currently pins the main path to F16 (the Metal bf16-load fix). CUDA *does*
//! support bf16, so a later pass may switch back to Bf16 on the CUDA lane for fidelity; F16 is
//! fine for this substrate check.

use std::time::Instant;

use luminal_cuda_lite::runtime::CudaRuntime;

use super::hf::prepare_hf_model;
use super::model::QwenConfig;
use super::target::QwenTarget;
use crate::pipeline::engine::{TargetChain, TokenId};

/// Median-ish ms per `verify` of chunk `[cur] + drafts` at `pos`, after a warm-up (which also
/// triggers the per-bucket kernel compile so it's not in the measured window).
fn verify_ms(t: &mut QwenTarget<CudaRuntime>, pos: u32, cur: TokenId, drafts: &[TokenId], reps: usize) -> f64 {
    let _ = t.verify(pos, cur, drafts); // warm-up / NVRTC compile
    let t0 = Instant::now();
    for _ in 0..reps {
        let _ = t.verify(pos, cur, drafts);
    }
    t0.elapsed().as_secs_f64() * 1000.0 / reps as f64
}

#[test]
#[ignore]
fn cuda_substrate_check() {
    let dir = prepare_hf_model("Qwen/Qwen3-0.6B").expect("download/prepare Qwen3-0.6B");
    let weights = dir.join("model_combined_bf16_v1.safetensors");
    // (1) Build + run: if we get past load(), Luminal CUDA + NVRTC works.
    let rt = CudaRuntime::new().expect("init CUDA runtime (toolkit + driver present?)");
    let mut t = QwenTarget::load(rt, QwenConfig::QWEN3_0_6B, weights.to_str().unwrap(), 768, 200);
    println!("\n[CUDA] Luminal CUDA loaded a 0.6B model (NVRTC ok).");

    // (2) Correctness: greedy 16 tokens; eyeball that they're plausible (not all-zero / garbage).
    t.reset();
    let prompt: Vec<TokenId> = vec![40, 1079, 264, 4108, 4108];
    let mut cur = t.prefill(&prompt);
    let mut out = vec![cur];
    let mut base = prompt.len() as u32;
    for _ in 0..16 {
        if t.is_eos(cur) {
            break;
        }
        cur = t.verify(base, cur, &[])[0];
        out.push(cur);
        base += 1;
    }
    println!("[CUDA] greedy 0.6B: {out:?}");

    // (3) Kernel cliff: build ~100 tokens of context, then time a single-token (s=1) vs an
    // 8-token (s=8) forward. On Metal s=8 ≈ 20× s=1 (the bug). Is CUDA the same, or flat?
    t.reset();
    let mut c = t.prefill(&[40]);
    let mut pos = 1u32;
    for _ in 0..100 {
        c = t.verify(pos, c, &[])[0];
        pos += 1;
    }
    let ms1 = verify_ms(&mut t, pos, c, &[], 5);
    let drafts = vec![c; 7];
    let ms8 = verify_ms(&mut t, pos, c, &drafts, 5);
    let ratio = ms8 / ms1.max(1e-6);
    println!("[CUDA] generic-bucket forward (ctx~{pos}): s=1 {ms1:.1} ms | s=8 {ms8:.1} ms | ratio {ratio:.1}×");
    if ratio > 3.0 {
        println!("[CUDA] => CLIFF EXISTS on CUDA too → Shard::load_chunked (specialized bucket) is needed in production (already implemented).");
    } else {
        println!("[CUDA] => NO cliff on CUDA → the generic multi-token kernel is fine; the s≥2 slowdown is Metal-only. load_chunked is not needed for the CUDA lane.");
    }
}
