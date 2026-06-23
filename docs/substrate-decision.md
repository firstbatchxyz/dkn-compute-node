# Substrate Decision Brief — DKN Pipeline-Parallel GLM-5.2 Serving

## 1. Scored Decision Matrix

Legend: ✓✓ strong / ✓ adequate / ~ partial-or-uncertain / ✗ absent-or-fights-you

| # | Criterion | llama.cpp + fork | **Luminal** | Candle | mistral.rs | torch+vLLM (shard) | MLX-only |
|---|---|---|---|---|---|---|---|
| 1 | Rust-native (no Python on node) | ✓✓ already embedded via llama-cpp-2 | ✓✓ pure Rust crate, static-link | ✓✓ pure Rust, smallest binary | ✓✓ Rust on candle | ✗ Python runtime per node, 2-process IPC | ✗ Python/Swift, Apple-only runtime |
| 2 | CUDA+Metal+ROCm coverage | ✓✓ **only one covering all 3** (+CPU/Vulkan) | ~ CUDA✓✓ Metal~ (no quant codegen) **ROCm✗** | ~ CUDA✓ Metal✓ ROCm=WIP BERT-only | ~ CUDA✓ Metal✓ ROCm✗ | ✗ CUDA/Blackwell-only | ✗ Metal-only |
| 3 | "Free cut" — run [a,b), inject/emit hidden state, no C++ fork | ~ feasible (prima.cpp proves it) but **is a C++ fork** | ✓✓ **best-in-class**: forward is a Rust `Vec<Layer>` loop; boundary = existing buffer round-trip | ✓ fork-the-model-file (Rust, medium) | ✗ monolithic candle modules; edit each model file + bypass device-mapper | ~ vLLM internals, deep edit | ~ edit Python model |
| 4 | Own KV → trivial KvTruncate/rollback | ✗ **`seq_rm` REFUSES partial removal on SWA/hybrid — M1-confirmed** | ✓✓ **you author the cache**; host page table is plain Rust; truncate = drop slots | ✓ you own tensors, manual | ✓✓ **`rollback_to`/`set_len`/`snapshot` already written**, incl. hybrid/rotating SWA | ✓ vLLM PagedAttention | ~ DIY |
| 5 | GLM-5.2-class MoE support / port cost | ✓ GGUF MoE runs; no 744B-NVFP4 path | ~ port: qwen3_moe + gemma4_moe (hybrid attn) templates, no kernels, **1–3 wk** | ~ port: no `glm4_moe`; good donors (qwen3_moe+deepseek2+glm4), medium | ✓✓ **native `glm4_moe.rs` + `glm4_moe_lite.rs` today** | ✓✓ **running 744B GLM-5.2 in production** | ~ DIY port |
| 6 | Quant adequate for 744B (int4/fp8/nvfp4) | ~ GGUF K-quants only; **no NVFP4** for frontier | ✓✓ **NVFP4 in HLIR (CUDA), matches modelopt/shard layout**; FP8 throughout. Metal/AMD: ✗ | ✓ GGUF int4; no native fp8/nvfp4 stack | ✓✓ **widest**: fp8/mxfp4/nvfp4/afq/gptq/awq/hqq/gguf/bnb + AFQ-on-Metal | ✓✓ NVFP4/CUTLASS MoE, proven at 744B | ~ Apple quant only |
| 7 | Dynamic-KV / autoregressive decode (paged attn) | ✓ works | ✓✓ symbolic dims + dim-buckets → compile once, `set_dim` per token; paged example | ✓ manual; no core paged attn | ✓✓ PagedAttention (CUDA + Apple) + prefix cache | ✓✓ PagedAttention | ✓ |
| 8 | Determinism (relative) | ✗ **Metal greedy NOT batching-det. — M1-proved**, worst end | ✓ seeded search → per-node fixed plan; no atomicAdd in reduces; cross-node ≠ bit-exact | ~ standard kernels, no guarantee | ~ candle math, no guarantee | ✓✓ **best demonstrated** (greedy-det + receipts @ NVFP4) | ~ |
| 9 | Maturity / production risk | ✓✓ battle-tested; but **fork = permanent rebase tax** | ~ v0.2.0, egglog rewrite, heavy CUDA tests (~495), pre-1.0 churn | ✓✓ 20.4k★, Cloudflare prod, foundational | ✓ 7.3k★, v0.8.12, very active | ✓✓ production-grade | ✓ Apple-niche |
| 10 | Team familiarity | ✓✓ **highest** — dkn-node ships it | ~ **conceptual** (andthattoo's fork) but **predates egglog rewrite**; `m_series_ops.rs`/`custom_passes.rs` won't port | ~ low (transitive via mistral.rs) | ~ low | ~ low | ✓ Apple ops in the fork |

**Honest uncertainty flags:** Luminal cross-node greedy acceptance `g` on quantized logits is **unmeasured** (the load-bearing unknown). mistral.rs's `rollback_to` on SWA caches is claimed-present but **untested by us under spec-decode rollback**. Luminal Metal has **zero FP8/FP4 kernels** (confirmed by dtype grep), not just "thin." vLLM frontier numbers are shard's, not ours.

---

## 2. The Recommendation — **Luminal (CUDA lane) for v1**

**Primary substrate: Luminal, standing up the NVIDIA-homogeneous GLM-5.2-NVFP4 chain first.**

The single clearest rationale: **the two load-bearing primitives for this entire project — the free cut (criterion 3) and an owned, trivially-truncatable KV cache (criterion 4) — are exactly the two things every other Rust/C++ substrate either forks for or fights you on, and Luminal makes both fall out for free because the model's forward is a plain Rust layer-loop and the KV cache is a `GraphTensor` you allocate yourself.** M1 already proved the KV problem is real and disqualifying for the obvious default: llama.cpp's `seq_rm` *refuses* partial removal on SWA/hybrid caches, and speculative-decode rollback is the load-bearing op. You cannot ship spec-decode on a cache the engine won't let you truncate. Luminal has no `seq_rm` to refuse — truncate is a host-side index drop on your own page table.

**Does the per-node-framework reality change the pick?** It *enables* this pick rather than changing it. The QUIC hidden-state contract means engines can differ per node, and your greedy-id-equality invariant means mixed-framework chains cost only acceptance-rate `g`, never correctness. That does two things: (a) it lets Luminal own the **NVIDIA frontier lane** — which is where 744B-NVFP4 GLM-5.2 *must* live anyway, because that's the only place the quant exists (criterion 6) and where draft+lm_head+accept are pinned regardless of this decision; and (b) it lets you keep llama.cpp/Vulkan as the untouched **AMD/heterogeneous fallback lane** without forcing one engine to cover the whole fleet.

**Why Luminal over the two strong challengers:**

- **vs. mistral.rs** (the "fastest to first token" option): mistral.rs wins criterion 5 outright — native `glm4_moe.rs` and an already-written `rollback_to` — and it is the correct pick *if speed-to-first-token is the only axis*. But it **loses the decisive criterion 3**: its models are monolithic candle modules with a built-in device-mapper, so exposing a hidden-state seam at an arbitrary layer boundary means forking and bypassing each model file — re-introducing exactly the "fork a project's internals and babysit it" tax you're trying to escape from llama.cpp. You'd be trading a C++ fork for a Rust fork. Luminal's seam is a supported buffer round-trip, not a fork. Over the project's horizon, the cut is touched on every model and every healing/rollback path; paying for it once structurally beats forking it per-model.

- **vs. torch+vLLM/shard** (the proven anchor): shard *is running your exact use case*, so the risk on criteria 5/6/8/9 is zero. But it fails criterion 1 hard (Python runtime + 2-process activation IPC on every node), inverts DKN's heterogeneous-consumer moat if it becomes the only substrate, and the team has low familiarity with vLLM/torch-dist internals. It stays as the **break-glass contingency**, not v1.

- **The team-familiarity caveat is real and I'm not hiding it:** andthattoo's hands-on Luminal experience predates the egglog compiler rewrite, so `m_series_ops.rs`/`custom_passes.rs` won't port mechanically — familiarity is conceptual, not code-level. But the *mental model* (author-the-graph, own-the-cache) is unchanged and is precisely the model this project needs, and the conceptual head-start still beats mistral.rs/vLLM where familiarity is near-zero.

---

## 3. Runner-Up + When to Switch

**Runner-up: mistral.rs.**

Switch to mistral.rs as the v1 engine if **any** of these fire:

1. **The cheapest experiment (§5) shows Luminal cross-node greedy acceptance `g` is too low** (or the FlashInfer per-step recapture overhead under growing context is prohibitive) — i.e. the Luminal-specific unknowns turn out badly. mistral.rs inherits candle's eager kernels, a different (possibly more forgiving) numeric profile, and gives you a second data point fast.
2. **Time-to-first-token becomes the binding constraint** (e.g. a demo/funding deadline). mistral.rs has the GLM-MoE model, the spec-decode subsystem, *and* the rollback KV already written — it is unambiguously the shortest path to a working chain, at the cost of a per-model cut fork.
3. **Luminal's pre-1.0 churn breaks you repeatedly** (criterion 9) — the egglog rewrite is recent and the API is unstable; if rebasing onto upstream becomes its own tax, mistral.rs on candle is the more stable Rust floor.

**Break-glass third: torch+vLLM/shard** — adopt *only* if 744B-NVFP4 must ship before the Luminal GLM port lands, accepting the Python-per-node and NVIDIA-only collapse for that lane.

---

## 4. What the Pick Costs

Concrete work to stand up GLM-5.2 on Luminal (CUDA lane):

1. **Model port (~1–3 wk):** new `glm5_2_moe/src/model.rs` from the `qwen3_moe` + `gemma4_moe` templates (78 layers, GLM RoPE/RMSNorm variant, 128+shared experts, group-routing, qk-norm, first-k-dense; **MTP head dropped per your decision**) + `hf.rs` weight-prep matching GLM tensor names. Pure graph ops — **no kernel work** since attention/RoPE/RMSNorm/MoE-gather all exist. Dominated by getting NVFP4 weight layout and RoPE/norm numerics bit-close (the 1141-LOC `qwen_bf16_repro.rs` shows how exacting parity tests are).
2. **Quant:** reuse `flux2/src/quant.rs` `Nvfp4Linear` (F4E2M1 packed + F8E4M3 block scale, block-16) — **matches modelopt/shard layout, no new quant infra** on CUDA.
3. **The cut:** declare `cx.named_tensor("hidden_in", (s, HIDDEN))`, loop `self.layers[a..b]`, mark boundary `.output()`; inject/emit via `set_buffer`/`get_f32` (the same round-trip already used for KV each step). Selective `load_safetensors` auto-subsets weights to the shard's layer range.
4. **KV rollback:** host page table is plain Rust; `KvTruncate{keep_to_pos}` = drop slots + overwrite `scatter_idx`. No `seq_rm`, no SWA refusal.
5. **Decode loop:** `dim_buckets('s', …)` compile once; `set_dim('s',1); set_dim('c', n+1)` per token. Budget the FlashInfer device-pointer-reuse subtlety (pre-size gather buffer) — a known, non-blocking concern.

**Biggest risk:** Luminal is **v0.2.0 with a freshly-rewritten egglog compiler and an unstable API** (criterion 9), and **cross-node greedy acceptance `g` on Luminal's quantized logits is unmeasured** (criterion 8 unknown). If `g` is too low, async pipelining doesn't pay and the throughput thesis collapses. Secondary risk: **Apple-Silicon nodes can't run NVFP4 on Luminal's Metal backend** (zero FP8/FP4 kernels) — but that's fine, because the frontier lane is NVIDIA-homogeneous by design and Apple/AMD stay on the llama.cpp fallback lane.

---

## 5. Cheapest Validating Experiment

**One test, built on the existing M1 harness: a 3-node hidden-state pipeline cut of a mid-size MoE (e.g. Qwen3-MoE, which Luminal already ships as a working example) in NVFP4 on Luminal-CUDA — NVFP4 frontier hardware NOT required, run it on whatever NVIDIA you have, even single-box with 3 processes over loopback QUIC first.**

Measure exactly two numbers that gate the whole recommendation:

1. **Cross-node greedy spec-decode acceptance `g`** between two *independently-searched* Luminal nodes (different compiled plans) — confirms Luminal's per-node-deterministic-but-not-bit-identical profile still clears the α≳0.83 acceptance bar your throughput math needs. (Shard proved this for NVFP4-on-Blackwell; it is **unknown for Luminal's quantized-logit path** and is the single highest-value unknown.)
2. **Per-token FlashInfer-recapture / set_dim overhead** under growing context `c` — confirms the compile-once/decode-with-set_dim model doesn't silently recompile or stall as context grows.

Why this is cheapest: it reuses Qwen3-MoE (zero model-port cost — Luminal ships it), reuses your M1 acceptance-measurement harness, needs no 744B weights and no Blackwell, and resolves *both* Luminal-specific unknowns (criteria 8 + 7) before you spend the 1–3 weeks on the GLM port. If `g` clears and recapture overhead is bounded, commit to Luminal. If either fails, fall to mistral.rs (§3) having spent days, not weeks.

---

## 6. Decision-Ready Summary

**Lock Luminal (CUDA lane) as the v1 substrate for the NVIDIA-homogeneous GLM-5.2-NVFP4 chain, with llama.cpp/Vulkan retained untouched as the AMD/heterogeneous fallback lane behind the same QUIC hidden-state contract.** Luminal wins because the project's two hardest primitives — the free layer cut and an owned, trivially-truncatable KV cache — are native and fork-free in Luminal, whereas llama.cpp structurally *refuses* partial KV truncation on SWA/hybrid caches (M1-confirmed) and mistral.rs/vLLM each re-introduce a per-model or Python fork to expose the cut. Before committing the 1–3-week GLM port, run **one** de-risking test: a 3-node Qwen3-MoE hidden-state cut on Luminal-CUDA measuring cross-node greedy acceptance `g` and per-token recapture overhead — the two unmeasured unknowns that gate the throughput thesis. If `g` clears ~0.83, build on Luminal; if it doesn't, fall back to mistral.rs (native `glm4_moe.rs` + ready-made `rollback_to` KV), accepting a per-model cut fork as the price of speed.