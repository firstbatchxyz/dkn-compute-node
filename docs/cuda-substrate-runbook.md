# CUDA substrate de-risk — cheap-GPU runbook

Goal: before renting Blackwell for NVFP4 + the cross-node `g` push, answer three cheap questions
on a low-cost NVIDIA GPU (the production substrate is `luminal_cuda_lite`, not Metal):

1. **Does Luminal CUDA build + run at all?** It compiles kernels at runtime via NVRTC, so it needs
   the **CUDA toolkit** (headers/libs), not just the driver. If `cargo build --features cuda`
   succeeds and a forward runs, the path works.
2. **Is the forward correct on CUDA?** (greedy a few tokens; eyeball them).
3. **Does the multi-token kernel cliff exist on CUDA?** On Metal the generic `[2,max]` `s` bucket
   is ~20× slower than `[1,1]` for s≥2 — the bug `Shard::load_chunked` works around. If CUDA has
   no cliff, that specialized bucket is a Metal-only workaround and the CUDA lane needs nothing.

## What to rent
- **Any modern NVIDIA GPU. NVFP4 is NOT needed** for this check (it runs F16) — so **no Blackwell**.
  An RTX 3090/4090, A10, or L4 is plenty (0.6B F16 ≈ 1.2 GB VRAM). Cheap (~$0.2–1/hr).
- Options: vast.ai / runpod / lambda / **JarvisLabs** (we have a `jarvislabs` skill — fastest path,
  can be driven from here).
- Need the **CUDA toolkit** image (e.g. `nvidia/cuda:12.x-devel`), not just `-runtime` (NVRTC
  needs the dev headers). Most GPU-cloud "CUDA dev" images have it.

## Setup on the box
```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env"
# CUDA toolkit present? (NVRTC is the load-bearing bit)
nvcc --version && nvidia-smi
# clone the fork (branch luminal-glm-pipeline)
git clone <repo> && cd dkn-compute-node && git checkout luminal-glm-pipeline
# hf-hub will fetch Qwen3-0.6B on first run; set HF_HOME if you want a specific cache dir
```

## Build (this alone answers Q1's "does it build")
```bash
cargo build --features cuda --tests 2>&1 | tee build.log
```
If `cudarc` / `luminal_cuda_lite` fails to find CUDA: ensure `CUDA_HOME`/`PATH` point at the
toolkit, and the image is `-devel` not `-runtime`. Fix CUDA-specific compile errors here (this
diagnostic was written on a Mac and could not be compiled against the CUDA backend).

## Run the diagnostic
```bash
cargo test --features cuda cuda_substrate_check -- --ignored --nocapture
```
Reads:
- Reaches `[CUDA] Luminal CUDA loaded a 0.6B model (NVRTC ok).`  → **Q1 yes** (substrate works).
- `[CUDA] greedy 0.6B: [...]` plausible token ids (not all-zero/garbage)  → **Q2 yes** (correct).
- `[CUDA] ... s=1 X ms | s=8 Y ms | ratio R×`:
  - `R > 3` → **cliff exists on CUDA** → `Shard::load_chunked` is needed in production (done).
  - `R ≈ 1` → **no cliff** → generic kernel is fine; the s≥2 slowdown was Metal-only.

## After this passes
- If no cliff and forward correct → the CUDA lane is healthy; graduate to **Blackwell** for the
  real work: Qwen3-MoE NVFP4 gating experiment + cross-node greedy `g` on quantized logits
  (`docs/substrate-decision.md` gating experiment).
- Optional fidelity pass: switch the CUDA lane's main dtype back to **Bf16** (Metal forced F16 for
  its missing bf16-load; CUDA supports bf16) and re-check the forward.
