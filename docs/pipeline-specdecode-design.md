# Design Brief: Node-to-Node Pipeline-Parallel Serving with Speculative Decoding for DKN

> **⚠️ REVISED 2026-06-19 after reading `leyten/shard` — read [`shard-adoption-brief.md`](shard-adoption-brief.md) alongside this. Two claims below are now CORRECTED:**
> 1. **§5 vocab-mismatch warning is WRONG.** GLM-4-9B *can* draft GLM-5.2. The 151552/154880 are padded `config.vocab_size` (weight-matrix rows), not the usable token-id space. Shard accepts via **exact integer token-id equality vs the target's greedy argmax** — vocab divergence can only cost speed (acceptance `g→0`), never corrupt output. **Use GLM-5.2 + GLM-4-9B draft directly; drop the MTP detour.** Caveat: this holds for **greedy decode only**; end-to-end temperature/top-p needs true speculative *sampling* (probability-ratio acceptance over aligned distributions).
> 2. **§1's synchronous one-traversal-per-step latency model is the SLOW regime (~3 tok/s).** Replace with shard's **async depth-N pipelined ring** (many verify chunks in flight) + **direct-return** (tail→coordinator in one hop): ~16 tok/s. This is the make-or-break perf lever and the biggest thing this brief underweighted.
> Also: keep the custom llama.cpp cut but **stage it** — eager draft + physical KV-crop first; defer the CUDA-graph static-KV-rollback draft (torch-specific, and unfinished even in shard). Lift shard's `topology.py` (Held-Karp min-latency ring) ~verbatim with attribution.

Author: staff eng. Audience: lead dev who will build this. Opinionated and concrete. Real symbols throughout.

---

## 1. Feasibility verdict & the core tension

**Verdict: a naive 20-deep pipeline over the public-internet WAN is NOT viable as a chat path. With a *local* speculative drafter it becomes viable, but 20 shards is the wrong number — target 6–10 hops, not 20.**

### The core latency problem

In pipeline parallelism the per-token critical path is one full traversal of the chain. With `N` shards there are `N−1` inter-node hops one-way; per-shard compute on a small layer-block is single-digit ms and is dominated by WAN latency. So:

```
L_traversal ≈ (N−1) · t₁ + Σ compute      where t₁ = RTT/2
T_standard  = num_tokens · L_traversal     (one traversal commits ONE token)
```

This is the wall. It is **not** bandwidth (the hidden state is ~12 KiB/token for GLM-5.2, §5) — it is serialized round-trips. The research on `ggml-rpc` makes the same point from the opposite direction: there the killer is thousands of synchronous control round-trips per token; here the killer is `N−1` data hops per token. Both are RTT-bound, not byte-bound.

### Worked math — standard decoding, N=20, RTT 50–150 ms

`L = (N−1)·(RTT/2) = 19·(RTT/2)`:

| RTT | t₁=RTT/2 | L/traversal | per-token | **tok/s** |
|----:|---------:|------------:|----------:|----------:|
| 50  | 25 ms | ~475 ms | ~475 ms | **~2.1** |
| 100 | 50 ms | ~950 ms | ~950 ms | **~1.05** |
| 150 | 75 ms | ~1425 ms | ~1425 ms | **~0.70** |

**Under 1 tok/s at 100 ms RTT. Unusable.** This is the central tension: the thing that makes a 20-deep WAN pipeline *possible* (only the tiny hidden state crosses each cut) does nothing for the thing that makes it *unusable* (you pay the whole chain's latency per token).

### How speculative decoding amortizes it

The draft model runs **entirely on node 0 (the path owner), off the WAN**. It proposes `γ` tokens locally. Those `γ` tokens are verified in **one** pipeline traversal (causal mask → target gives `q(·)` at all `γ+1` positions in a single forward pass — this is exactly what `validate_prefill()` already does today). So:

```
T_spec = num_traversals · L_traversal,  where each traversal commits E[accepted] tokens
E[accepted] = (1 − α^(γ+1)) / (1 − α)     (truncated geometric, α = acceptance rate)
```

The WAN term `(N−1)·t₁` is now paid **once per ~E[accepted] tokens** instead of once per token. The speedup over standard decoding **equals E[accepted]**, because both pay the same per-traversal `L` and the draft is free (local).

### Worked math — speculative, γ=8, N=20

Effective per-token latency = `L / E[accepted]`:

| α | E[accepted] (γ=8) | tok/s @50ms (L≈475) | tok/s @100ms (L≈950) | tok/s @150ms (L≈1425) |
|----:|----:|----:|----:|----:|
| 0.5 | 1.99 | 4.2 | 2.1 | 1.4 |
| 0.7 | 3.19 | 6.7 | 3.4 | 2.2 |
| 0.8 | 4.16 | 8.8 | 4.4 | 2.9 |
| 0.9 | 5.70 | 12.0 | 6.0 | 4.0 |
| 0.95 | 6.63 | 14.0 | 7.0 | 4.6 |

**Design threshold:** to clear a ~5 tok/s interactive floor at 100 ms RTT you need `E[accepted] ≳ 4.75`, i.e. **α ≳ 0.83 with γ=8**. The lever is **acceptance rate** (a high-quality, same-tokenizer draft — §5), not big γ: at α≈0.8, going γ 8→16 lifts E[accepted] only ~4.16→4.8. Set **γ ∈ [8,16]** and invest everything in α.

### 20 is the wrong number

Latency scales linearly in `N`; failure probability of the chain scales roughly as `1−(1−p)^N` (§7). Both push you to **minimize N**. The only thing pushing N up is per-shard VRAM. From §5: GLM-5.2 at **Q4 ≈ 19 GB/shard at 20 shards**, but **≈ 38 GB/shard at 10 shards** and **≈ 63 GB/shard at 6 shards**. So:

- **Target N = 6–10**, gating on the VRAM tier you can actually recruit. 10× 48 GB cards (Q4, ~38 GB/shard) halves `L` vs 20× 24 GB cards and roughly halves chain-break probability.
- The DSD paper's sweet spot is `3 ≤ N ≤ 8`. **Treat 20 as the absolute ceiling, 6–10 as the design point.** Make N a function of recruited-node VRAM, not a constant.

---

## 2. Recommended architecture

**Build the hidden-state cut yourself. Do NOT use ggml-rpc.** (Justification below, then the design.)

### Build-on-rpc vs custom: custom, decisively

From research[0]: ggml-rpc is fatal here on four axes — (1) latency model is `O(thousands of control round-trips) × RTT` per token because every temporary tensor triggers a synchronous `INIT_TENSOR`/`SET_TENSOR`/`GET_TENSOR` round-trip with no batching; (2) "**Never run the RPC server on an open network**" — zero auth, attacker-deserialized graphs are RCE-class; (3) it's a central-host star, not autonomous peers each owning a layer block with local KV; (4) `llama-cpp-2 = 0.1.137` exposes **no `rpc` feature** and no FFI for `ggml_backend_rpc_init` — you'd fork `-sys` anyway.

The custom cut ships **one small message per stage per token** (~12 KiB), gives genuine per-request pipeline overlap, per-node KV locality, and a trust/auth model fit for the public internet. The hard part is real but bounded: exposing per-block **"prefill→hidden-state"** and **"resume-from-hidden-state→logits"** entry points by splitting llama.cpp's graph at layer boundaries (prima.cpp is the existence proof). This is the one genuine new piece of llama.cpp/ggml work.

### Roles in the chain

```
        ┌─ DRAFT (GLM-4.7-Flash or MTP head, local) ─┐
        │                                            │
 node 0 ── shard 0 ── shard 1 ── … ── shard N-1 ──┐
 (PATH OWNER,        [layers L0]   [layers Lk]    │
  embed + draft +     +KV slice    +KV slice +    │
  accept/reject)                   norm + lm_head │
        ▲────────── logits/hidden return ─────────┘
```

- **Node 0 = path owner / orchestrator.** Holds `embed_tokens`, runs the **draft model locally**, owns the autoregressive loop, runs the accept/reject (modified rejection sampling), holds the canonical committed token sequence. Also owns shard 0's layers (or not — keep it flexible; embedding + draft + shard-0 can co-reside or split).
- **Shards 1..N−2** hold contiguous layer blocks + their own KV slice. Pure compute hops.
- **Shard N−1 (tail)** holds the last layer block, `final_norm`, `lm_head`. It is the only shard that projects to logits. For spec-decode it must return **per-position logits/logprobs for all γ+1 verify positions** (not a sampled token — the accept/reject happens on node 0 where the draft distribution `p(·)` lives).

This mirrors dnet's `is_first_layer`/`is_last_layer`/`next_instance` split and exo's `Shard{start_layer,end_layer}` — but the sampler moves to **node 0**, not the tail, because verification needs both `p` (draft, on 0) and `q` (target, from tail) co-located.

### One generation step (the loop on node 0)

1. **Draft γ tokens locally.** Draft model on node 0 autoregressively produces `x₁…x_γ` + per-token distributions `p(·)`. No WAN.
2. **One pipeline-verify traversal.** Node 0 embeds `[last_committed, x₁…x_γ]`, sends the hidden-state frame into shard 0. Each shard runs its layers over the `γ+1`-token batch (advancing its KV by γ+1 *tentatively* — see KV below), forwards the hidden state. Tail computes logits at all `γ+1` positions → returns **`q(·)` logprobs** (or top-k) to node 0.
3. **Accept/reject on node 0.** For each `xᵢ` left-to-right: accept w.p. `min(1, q(xᵢ)/p(xᵢ))`; on first rejection, resample from `norm(max(0,q−p))` and discard the rest; if all accept, sample a bonus token from `q` at position γ+1. Commit **1..γ+1** tokens.
4. **KV reconciliation.** Tell shards how many positions to *keep* (`n_accepted`) and roll back the rest. Repeat from 1.

This is the DSD `T_DSD = k·t₀ + (N−1)·t₁` regime: the `(N−1)·t₁` WAN cost is paid once per step, committing `E[accepted]` tokens.

### KV cache placement

- **Per shard, per request, local — never crosses the wire.** Exactly the dnet/exo model (`_kv_by_nonce`, `poll_state(request_id)`). Maps onto DKN's existing per-request `LlamaContext` rebuilt in `spawn_blocking` (the engine already allocates `ctx_size = prompt + max_tokens`). Each shard's context holds only K/V for *its* layers.
- **Speculative rollback is the new requirement.** A shard must advance its KV by γ+1 on verify, then be told to truncate to `+n_accepted`. llama.cpp exposes this via `llama_kv_cache_seq_rm` / `seq_rm` on a sequence's positions `[n_accepted, γ+1)`. This is the single hardest KV change and must be a first-class protocol op (`KvTruncate { request_id, keep_to_pos }`). dnet/exo have **no** rollback — this is greenfield.

### Return path for logits

Tail → node 0 directly (not back through the chain). dnet does this with a `callback_url` and `SendToken`; you'll do it as a QUIC stream from tail to node 0 carrying **`{request_id, step, positions: [γ+1 logprob vectors or top-k]}`**. Node 0 already has the bidi stream to the router for token streaming to the user; the tail→0 path is a *new* node-to-node stream.

---

## 3. The node-to-node transport

**Recommendation: reuse the existing QUIC/quinn + length-prefixed msgpack stack. Do not introduce gRPC.**

### QUIC vs gRPC

dnet/exo use plaintext gRPC over `insecure_channel` — every prior-art report flags this as a WAN non-starter (no TLS, no auth, no NAT traversal, gRPC default 4 MB / idle-GOAWAY problems, and in dnet the 64 MB/keepalive options aren't even applied to the hop channel). You already have the better stack:

- `RouterConnection::connect` (`src/network/connection.rs`) — QUIC endpoint, bi-stream, `accept_bi()` pattern, `SkipServerVerification` for `--insecure`.
- `read_framed`/`write_framed` + `MAX_MESSAGE_SIZE = 10MB` (`dkn-protocol/src/framing.rs`) — length-prefixed msgpack.
- secp256k1 challenge-response (`src/network/auth.rs::authenticate`, `dkn-protocol/src/auth.rs`).

Adding gRPC means a second TLS stack, a second auth model, prost/tonic, and proto churn — for zero benefit over quinn bidi streams. **Lift dnet's *stream semantics* (per-request stream keyed by nonce, seq-numbered frames, ACK reader, idle sweeper, backpressure-driven link disable — its `StreamManager`/`StreamContext`), reimplemented over quinn.** That is the reusable idea; gRPC the wire is not.

### NAT traversal — the real new problem

Today everything is node↔router star; there is **no** node↔node link, and consumer nodes are behind NAT. dnet/exo assume direct LAN reachability and fail here. Your options, in order:

1. **Router-coordinated QUIC hole-punching (recommended primary).** The router is already the rendezvous every node holds a QUIC connection to. Reuse it as a STUN-like coordinator: it knows each node's observed `SocketAddr`, and orchestrates simultaneous-open between consecutive shards `(i, i+1)`. QUIC over UDP hole-punches better than TCP. New protocol: `PunchRequest`/`PunchCoordinate` carrying each peer's candidate addrs + a synchronized punch time.
2. **Router/relay fallback (must-have).** When hole-punching fails (symmetric NAT, CGNAT — a large fraction of consumer nodes), relay the hidden-state frame through a relay node or the router itself. Bandwidth is trivial (~12 KiB/token × steps), so relay cost is acceptable as a fallback. **You need this fallback or chains won't form for a big slice of the network.**
3. **Public-IP / port-forwarded nodes as "spine".** Prefer nodes with a routable inbound addr for the high-fanout middle of chains; treat NAT'd nodes as endpoints (node 0 / tail) where possible. Advertise reachability as a capability (§6).

**The router stays out of the data path once the chain is punched** — it's matchmaker + fallback relay only. This preserves the star for control while adding a mesh for data.

### What's on the wire

Per shard→shard hop, per step (GLM-5.2, H=6144):

| Field | Value |
|---|---|
| hidden-state tensor | `[γ+1, 6144]` |
| dtype on wire | **bf16 = 12.0 KiB/token**, or fp8 = 6.0 KiB/token, or Q8-block (dnet's trick) ≈ 6.3 KiB/token |
| frame metadata | `request_id`, `step`, `seq`, `layer_id` (= boundary index), `shape`, `dtype`, `n_tokens` |

**Compression: ship Q8-block-quantized hidden states (dnet's `NnBlockQ80` / qsparse8 idea), not raw fp16.** It halves per-hop bytes for a known small accuracy cost; the dtype-string-as-metadata trick (`"float16|fmt=qsparse8_v1|gs=64"`) lets you evolve the codec without a schema bump. But note: at 12 KiB/token bandwidth is **not** your bottleneck (RTT is), so compression is a second-order optimization — do it for asymmetric home uplinks, not for latency.

### Reliability

QUIC gives you in-order, reliable, encrypted streams per request — you do **not** re-implement dnet's seq/ACK reordering. Use **one quinn bidi stream per (request, hop)**, keyed by `request_id`. Add app-level credits/backpressure (dnet's weakest area) so a slow tail can't unbounded-buffer node 0. Idle-sweep stale request streams (dnet's `cleanup_idle_streams`).

---

## 4. What to lift from dnet vs build new

| dnet asset | Verdict | Notes |
|---|---|---|
| **Ring/activation proto** `Activation{data, shape, dtype, layer_id, batch_size}` + per-request decode params | **Lift the schema, re-encode as msgpack** | Mirror as a Rust `HiddenStateFrame` in `dkn-protocol`. Add `request_id`, `step`, `n_accepted` (rollback). Drop protobuf/gRPC. |
| **`StreamManager`/`StreamContext`** (per-nonce stream, ACK reader, idle sweep, backpressure link-disable) | **Lift the *design*, reimplement over quinn** | This is the single most reusable transport idea. |
| **`compute_layer_assignments` / `postprocess_single_round`** (contiguous ranges + next_instance wrap, single-layer-device cleanup) | **Lift the algorithm** | Drop the ring wrap (you want a *chain*, tail→0 only for logits). Port to a Rust `PipelinePlan`. |
| **`abs_to_local` / `BaseRingModel.load_weights` key-remap** (`model.layers.N → local.LOCAL`) | **Lift as a template** for GGUF layer-range loading | Concept ports; the MLX loader does not. This is the llama.cpp graph-surgery work. |
| **`compression/wire.py`** sparse_v1 / qsparse8_v1 + dtype-string metadata | **Lift the algorithm + the metadata trick** | Reimplement in Rust/CPU. The **MLX Metal kernels are Apple-only — drop.** |
| **Memory-weighted partitioning** (exo `fraction = mem/total_mem`) | **Lift as the default heuristic** | Upgrade to also weight by VRAM tier + link RTT for chain ordering. |
| **`get_or_make_kv` + TTL** per-request KV | **Lift the pattern** | You already have per-request contexts; add the **rollback** op (new). |
| **MLX runtime** (`apply_single_layer`, `make_cache`, Metal kernels, `mx.eval`) | **Do NOT port** | Apple-Silicon/UMA-only. Your compute is llama.cpp/GGUF. |
| **mDNS discovery, Thunderbolt scrape, `local_ip`/`is_private()` LAN filter** (dnet-p2p) | **Do NOT port** | LAN-only, NAT-hostile. Discovery is the router + Redis inventory you already have. |
| **HALDA MILP placement** | **Do NOT port as-is** | Assumes a static profiled device set. Your placement is dynamic/churny — build a lighter greedy/online placer in the router (§6). |
| **Single-token `send_tokens`/`await_token` driver** | **Lift the *correctness* reference**, not the shape | Your driver is spec-decode (γ tokens/traversal), which dnet/exo/distributed-llama/exo **all lack** — every prior-art spec-decode section returned *nothing*. This is greenfield. |

**Net: you lift schemas, the stream-lifecycle design, the layer-assignment math, the compression algorithm, and the KV-per-request pattern. You build new: the llama.cpp graph cut + resume entry points, KV rollback, the spec-decode driver, NAT traversal, and per-shard validation/metering.**

---

## 5. Speculative decoding specifics

### Tokenizer compatibility verdict for GLM — **the user's stated plan is broken**

The user said: node 0 runs **GLM-4 9B** as the draft for a big GLM target. **This does not work.** From research[2]:

- GLM-4.x family (GLM-4-9B, GLM-4.6): **vocab 151552**.
- GLM-5 family (GLM-5 / 5.1 / **5.2**, and GLM-4.7-Flash): **vocab 154880**.

`llama.cpp`'s `common_speculative_are_compatible()` hard-requires: same vocab type, same BOS/EOS, **vocab sizes within 128**, and **every token id ≥5 must `strcmp`-match**. `|154880 − 151552| = 3328 ≫ 128` → **rejected**. GLM-4-9B as drafter would corrupt verification and silently break the lossless guarantee.

**Decision fork (this is an Open Decision, §9):**

- **If target = GLM-5.2 (vocab 154880):** draft options, in order —
  1. **GLM-5.2's built-in MTP head** (`num_nextn_predict_layers: 1`, +20% acceptance length, co-trained, zero vocab risk). **Recommended.** Pin it to the **tail shard** alongside `lm_head` so drafts are produced where the final hidden state already lands. *Caveat:* MTP-as-drafter changes the topology — the draft is no longer purely on node 0; see below.
  2. **GLM-4.7-Flash** (vocab 154880, `glm4_moe_lite`, 47 layers, ~30B lite MoE) as a **standalone external drafter on node 0**. This is the clean "draft local on node 0" design. Bigger than 9B but tokenizer-correct.
- **If target = GLM-4.6 (vocab 151552):** then **GLM-4-9B *is* tokenizer-compatible** and works as the user originally wanted. This is the simplest correct path if you can accept the older/larger-layer target (92 layers, H=5120).

**My opinion:** start with **GLM-4.6 target + GLM-4-9B draft** for the prototype (the user's pairing, and it's tokenizer-valid), because it lets you validate the whole pipeline+spec machinery with a known-compatible pair. Move to **GLM-5.2 target + GLM-4.7-Flash draft** (or MTP) for production, where 154880 must match 154880.

### Where the draft runs

**On node 0, off the WAN — non-negotiable.** If the draft were itself sharded across the WAN, drafting γ tokens would re-incur γ serialized traversals and the entire amortization vanishes (research[1]). The draft must fit on one node. GLM-4-9B (~9 GB Q8) and GLM-4.7-Flash both qualify. The MTP-head option is the exception: it lives on the tail, which means the *draft* result must come back to node 0 before the verify traversal — adds one tail→0 hop to the draft, not γ hops, so it's tolerable but makes node 0 not strictly self-sufficient. Prefer a standalone node-0-local drafter unless MTP's +20% α is decisive.

### Acceptance across a sharded target

The target's `q(·)` is produced only at the **tail** (it owns `lm_head`). The draft's `p(·)` lives on **node 0**. So:

- Tail returns **per-position `q` logprobs (or top-k)** for all γ+1 verify positions — *not* a sampled token.
- Node 0 runs the modified rejection sampling (it has both `p` and `q`). This is why the sampler is on node 0, not the tail (contrast dnet, where the tail samples).
- `validate_prefill()` (`src/inference/engine.rs:562`) is **already the verify primitive**: one forward pass over prompt+output, extract per-position logprobs at stride positions (`extract_logprob` at `:678`). Extend it to: (a) run *only* the tail's layer block from an incoming hidden state, and (b) return logprobs at *every* verify position, not every-N. The accept test `min(1, q/p)` is new code on node 0.

### Protocol additions for spec-decode

- `DraftProposal { request_id, step, tokens: [γ], p_logprobs: [γ × top_k] }` — internal to node 0, but the **p_logprobs must accompany** the verify if you ever split draft/accept across nodes; keep on node 0 for now.
- Tail→0 `VerifyResult { request_id, step, q_logprobs: [(γ+1) × top_k] }`.
- Chain op `KvTruncate { request_id, keep_to_pos }` — the rollback, broadcast to all shards after accept/reject.
- Reuse `logprob_top_k` / the `InferenceProof`/`TokenLogprob` types (`dkn-protocol/src/proof.rs`) for the q-logprob payload — they already exist for validation.

---

## 6. Protocol & router changes

### New `dkn-protocol` message types

In `dkn-protocol/src/message.rs`, extend `RouterMessage` and `NodeMessage` (and add a new node↔node enum):

```rust
// Router -> Node (path formation)
RouterMessage::PipelineAssignment {
    request_id: Uuid,
    model: String,
    role: ShardRole,                 // Head { draft_model } | Middle | Tail { has_lm_head }
    layer_range: (u32, u32),         // contiguous [start, end)
    next_hop: Option<NodeAddr>,      // shard i -> i+1 candidate addrs (for punch)
    prev_hop: Option<NodeAddr>,
    owner_addr: NodeAddr,            // node 0, for tail->0 logits return
    plan_epoch: u64,                 // bump on re-route
}
RouterMessage::PunchCoordinate { request_id, peer: NodeAddr, candidates: Vec<SocketAddr>, punch_at_unix_ms: u64 }
RouterMessage::PipelineTeardown { request_id }

// Node -> Router (capability + path lifecycle)
NodeMessage::ShardCapability {           // extend StatusUpdate or new variant
    vram_free_gb: f32, gpu_kind: String, reachable: Reachability, // PublicIp|Punchable|RelayOnly
    supports_layer_split: bool, link_rtt_hint_ms: Option<u32>,
}
NodeMessage::PunchRequest { request_id, observed_addr: SocketAddr }
NodeMessage::ShardReady { request_id, layer_range: (u32,u32) }
NodeMessage::ShardFailed { request_id, layer_range: (u32,u32), error: String }

// NEW node <-> node enum (carried over the punched QUIC stream, framed by dkn-protocol framing)
enum ShardMessage {
    HiddenStateFrame { request_id, step, seq, layer_id: u32, shape: [u32;2], dtype: String, data: Vec<u8> },
    VerifyResult     { request_id, step, q_logprobs: Vec<TokenLogprob> },  // tail -> owner
    KvTruncate       { request_id, keep_to_pos: u32 },
    ShardAck         { request_id, seq },
}
```

Reuse `read_framed`/`write_framed`/`MAX_MESSAGE_SIZE` for `ShardMessage` — 12 KiB frames are nowhere near the 10 MB cap.

### Router changes (`dkn-router`)

- **Path builder (new module, `src/pipeline.rs`).** Greedy/online placer: given a model needing layers `[0,L)` and the set of nodes for that model, pick a chain minimizing `Σ link_rtt` and `N` subject to per-node VRAM. Reuse `NodePool::find_nodes_for_model` (`node_pool.rs:153`) for the candidate set; extend `NodeHandle` (`node_pool.rs:21`) with VRAM/reachability/RTT from `ShardCapability`. Emit `PipelineAssignment` to each shard via `NodePool::get_sender`. **Replace dnet's static MILP with this online placer** — churn forbids static plans.
- **Rendezvous/punch coordinator** in `quic_server.rs` (`handle_node_message`, `:279`): handle `PunchRequest` → `PunchCoordinate`; fall back to relay (route `HiddenStateFrame` through the router or a relay node) when both peers report `RelayOnly`.
- **Dispatch.** `TaskRouter::dispatch_stream_internal` (`task.rs:570`) currently dispatches to **one** node. Add `dispatch_pipeline()`: build plan, assign shards, then dispatch the *prompt* to node 0 only; node 0 drives the spec-decode loop and streams tokens back exactly as a single node does today (so `StreamToken`/`StreamEnd`/`StreamError` over the existing router stream are unchanged from the router's POV — the chain is invisible above node 0).
- **Metering per-shard.** `meter_output`/`MeterReport` (`meter.rs:23`) meters one node today. Add `meter_pipeline()`: the committed token count (from node 0's `StreamEnd.stats`) is split across shards by **layer-count weight** (`layer_range` size). Record a `NodeStatsEntry` (`task.rs:183`) per shard. Settlement credits all N shards for the request, weighted by layers held (and optionally by measured uptime in the chain).

### Validation / proof-of-inference under a chain — the hard part

Today: router dispatches a `ValidationTask` to a **different** node with the same model, which re-runs `validate_prefill()`, and `trust.rs::compare_proofs` (`:148`) / `compare_to_consensus` (`:212`) checks logprob agreement. **A chain breaks this** because no single node can re-run the whole model.

Two-tier validation:

1. **End-to-end output validation (cheap, primary).** Treat the *whole chain's output* as if it came from one logical node "model=GLM-5.2". Dispatch a `ValidationTask` for the (prompt, output) pair to **another full chain** (or to any single node that can host the model if one exists) and compare logprobs via existing `compare_proofs`. The chain is a black box; you validate its output distribution. This reuses **all** existing trust machinery. **Do this first.**
2. **Per-shard contribution proof (for crediting/slashing individual shards).** Each shard commits to a hash of `(input_hidden_state, output_hidden_state, layer_range, request_id, step)` — extend `InferenceProof`/`kv_cache_hash_placeholder` (`engine.rs:732`). A validator can re-run a *single shard's* layer range given the recorded input hidden state and check the output hash. This catches a lying middle shard without re-running the whole model. It's the new primitive (no prior art has it). Make it **probabilistic** (spot-check a random shard per request, à la the existing adaptive validation rate `compute_validation_rate`).

`TrustScorer` (`trust.rs:242`) and the per-address Redis trust state extend naturally: each shard address accrues honesty independently from the spot-checks.

---

## 7. Fault tolerance & economics

### The chain-break problem

With per-node failure probability `p` over a request's duration, chain success ≈ `(1−p)^N`. At p=2% per node: N=20 → **67%** success; N=10 → **82%**; N=6 → **89%**. This alone justifies §1's "minimize N." dnet/exo/distributed-llama all have **essentially zero** fault tolerance — a single drop stalls to timeout. You must do better.

### Redundancy / re-route strategy

- **Hot standby per shard (recommended).** The path builder assigns a **primary + 1 standby** per layer range (standby holds the same layers, warm, KV-empty). On a missed `ShardAck`/heartbeat within a deadline, node 0 (or the router) promotes the standby. **KV is the catch:** the standby has no KV for in-flight requests. Recovery = **re-prefill the standby** for that request's committed prefix (replay the committed tokens through the standby's layers). This is cheap relative to losing the whole request and is *only possible because spec-decode keeps node 0's canonical committed-token sequence* — node 0 can always reconstruct any shard's required state by replaying committed tokens. **This is a major architectural advantage of putting the canonical sequence on node 0.**
- **Checkpoint = the committed token list on node 0.** No need to migrate KV across the wire (research confirms nobody does this well). Reconstruct, don't migrate.
- **Re-route epoch.** `plan_epoch` on `PipelineAssignment` bumps on any promotion; stale frames (old epoch) are dropped. Bound recovery to a few hundred ms (one re-prefill traversal).
- **Tiered N by reliability.** Build shorter chains (N=6) from high-uptime/public-IP nodes for latency-sensitive traffic; longer chains (N up to ceiling) only when VRAM forces it.

### Economics — per-shard crediting

- **Credit split by layers held**, from §6's `meter_pipeline()`: shard `i` earns `(layers_i / L) × committed_tokens × rate`, gated on passing the per-shard contribution spot-check (§6 tier 2). A shard that fails its hash spot-check is **slashed** and greylisted (`TaskRouter::greylist_address`, `task.rs:449`) — reuse the existing greylist path.
- **Standby credit.** Pay standbys a small retainer for warmth; pay full rate only when promoted-and-contributing. Otherwise nobody volunteers to be a standby.
- **Node 0 premium.** The path owner does draft compute + orchestration + accept/reject; credit it for draft tokens *and* an orchestration fee, not just its layer slice.
- **Validation under chains** (from §6): end-to-end output validation feeds the existing `TrustScorer`/settlement for the *logical model*; per-shard spot-checks feed each shard address's trust independently. Settlement only pays a request once the end-to-end output passes (or passes the adaptive sampling gate), exactly as today — the chain is metered as one logical producer at the settlement layer, then split internally.

---

## 8. Phased build plan

Each phase independently testable. Symbols are real and confirmed in the three repos.

**Phase 0 — Two nodes pass one hidden state over QUIC (no inference).**
Touch: `src/network/connection.rs` (generalize `RouterConnection::connect` → a `PeerConnection` that can both `connect`+`open_bi` and `accept_bi`); new `src/network/peer.rs`; `dkn-protocol/src/message.rs` (`ShardMessage::HiddenStateFrame`); reuse `framing.rs`. Test: node A sends a `[γ+1, 6144]` bf16 buffer to node B, B echoes, roundtrip asserted. *Exit:* one frame over real QUIC between two processes.

**Phase 1 — llama.cpp graph cut: prefill→hidden-state and resume→logits.**
Touch: `src/inference/engine.rs`. Add `InferenceEngine::load_shard(layer_range)` (load only assigned GGUF layers — the `abs_to_local` template), `forward_hidden(input_hidden, n_tokens) -> output_hidden` (run only owned layers, advance local KV), and on the tail `project_logits(input_hidden) -> Vec<TokenLogprob>` (reuse `extract_logprob` `:678`). This is the hard llama.cpp/ggml work. Test: shard-0 + shard-1 on **one machine** reproduce a single model's logits within tolerance vs the unsharded `generate()`.

**Phase 2 — Two-node pipeline, standard (non-spec) decode, over the network.**
Touch: `src/worker.rs` (new `run_pipeline_head` / `run_pipeline_shard` alongside `run_inference` `:325`); `src/network/peer.rs`. Node 0 embeds, sends hidden → shard 1 (tail) projects → returns logits → node 0 samples 1 token → repeat. Test: two processes on two machines produce coherent text. *This is the slow (~1 tok/s) baseline — confirm correctness, measure `L_traversal`.*

**Phase 3 — Speculative decode on the 2-node pipeline.**
Touch: `src/inference/engine.rs` (extend `validate_prefill` `:562` → `verify_positions` returning all-position logprobs; add `draft_step` using a second `InferenceEngine` for the draft model); `src/worker.rs` (accept/reject loop on node 0); `dkn-protocol` (`VerifyResult`, `KvTruncate`). Implement KV rollback via `llama_kv_cache_seq_rm`. Test: GLM-4.6 target + **GLM-4-9B draft** (tokenizer-valid pair, §5) — assert spec-decode output == standard-decode output (lossless), measure `E[accepted]` and the tok/s lift vs Phase 2.

**Phase 4 — N-node chain + router path formation + NAT punch.**
Touch: `dkn-router/src/pipeline.rs` (new path builder); `node_pool.rs` (`NodeHandle` + `ShardCapability` fields, extend `find_nodes_for_model`); `quic_server.rs::handle_node_message` (`:279`, `PunchRequest`/`PunchCoordinate` + relay fallback); `task.rs` (`dispatch_pipeline` alongside `dispatch_stream_internal` `:570`); `dkn-protocol` (`PipelineAssignment`, punch msgs). Test: 6-node chain across 6 NAT'd machines forms via router-coordinated punch (relay fallback on symmetric NAT), runs spec-decode end-to-end, streams to user via the unchanged router stream.

**Phase 5 — Fault tolerance: standby + re-prefill recovery.**
Touch: `dkn-router/src/pipeline.rs` (primary+standby assignment, `plan_epoch`); `src/worker.rs` (heartbeat/ack deadline, promote-and-re-prefill from node 0's committed sequence); `dkn-protocol` (`ShardFailed`, `PipelineTeardown`). Test: kill a middle shard mid-generation; assert request completes via standby within bounded recovery, output still lossless.

**Phase 6 — Validation + metering under chains.**
Touch: `dkn-router/src/trust.rs` (per-shard contribution spot-check using extended `InferenceProof`; reuse `compare_proofs` `:148` for end-to-end output validation against a second chain); `meter.rs` (`meter_pipeline`, layer-weighted split); `task.rs` (`NodeStatsEntry` per shard `:183`, settlement split, slash-on-failed-spot-check via `greylist_address` `:449`); `engine.rs:732` (real per-shard hash, not `kv_cache_hash_placeholder`). Test: a lying middle shard (perturbs hidden state) is caught by spot-check and slashed; honest shards are credited proportionally; end-to-end output validation passes for honest chains.

**Phase 7 — Production GLM-5.2 + GLM-4.7-Flash (or MTP) draft, full N, tuned.**
Touch: `dkn-router` model registry / `ModelRegistryEntry` (add `draft_model` companion field, tokenizer-compatibility gate equivalent to `common_speculative_are_compatible`); tune γ ∈ [8,16], N ∈ [6,10] by recruited VRAM; enable Q8-block wire compression (`HiddenStateFrame.dtype` metadata). Test: GLM-5.2 sharded across N production nodes, spec-decode with vocab-154880 draft, validated + metered, ≥5 tok/s at representative RTT.

---

## 9. Open decisions for the user (these genuinely fork the architecture)

1. **Model identity & draft pairing — decides tokenizer correctness.** GLM-4-9B **cannot** draft for GLM-5.2 (vocab 151552 ≠ 154880). Choose:
   (a) **GLM-4.6 target + GLM-4-9B draft** (your original pairing, tokenizer-valid, simplest prototype), or
   (b) **GLM-5.2 target + GLM-4.7-Flash draft** (both 154880, production), or
   (c) **GLM-5.2 + built-in MTP head** (best α, but draft lives on the tail, not node 0, changing the topology). My recommendation: (a) for Phases 0–6, (b) for Phase 7.

2. **Shard count N.** 20 is the wrong target for WAN (≤1 tok/s, 67% chain-success at p=2%). I recommend **N=6–10 gated on recruited VRAM tier** (Q4: ~63 GB/shard at 6, ~38 at 10, ~19 at 20). Are you willing to require 48 GB+ nodes for the middle of chains to keep N small, or is the network mostly 24 GB consumer cards (forcing N≈20 and a hard dependence on high α)?

3. **NAT-traversal posture.** Punch-first with router-relay fallback is what I recommend. Do you accept the router (or designated relay nodes) carrying hidden-state frames for `RelayOnly`/symmetric-NAT nodes — or do you want to *exclude* un-punchable nodes from chains entirely (fewer eligible nodes, simpler data plane)?

4. **Validation granularity.** End-to-end output validation (reuses all existing trust machinery, but can't attribute a bad shard) vs adding per-shard hash spot-checks (attributes + slashes individual shards, new primitive). Ship E2E-only first, or build per-shard attribution from the start because crediting/slashing 6–20 independent operators per request demands it?

5. **Draft topology when using MTP.** If you want GLM-5.2's MTP head (+20% α) instead of a standalone node-0 drafter, node 0 is no longer self-sufficient (draft result must return from the tail before each verify). Accept that extra tail→0 hop on the draft for the α gain, or keep the draft strictly node-0-local with GLM-4.7-Flash (cleaner recovery story, slightly lower α)?

---

### Confirmed anchor symbols (for the implementer)
- Verify primitive: `InferenceEngine::validate_prefill` / `extract_logprob` / `kv_cache_hash_placeholder` — `src/inference/engine.rs:562,678,732`. Inference: `generate` `:235`, `load` `:104`.
- Worker: `Worker::try_accept` `:65`, `run_inference` `:325`, `run_validation` `:473`, `add_engine`/`remove_engine`/`has_model` `:256,264,269`.
- Transport: `RouterConnection::connect`/`accept_bi` — `src/network/connection.rs:33,57`; `authenticate` — `src/network/auth.rs:19`.
- Protocol: `NodeMessage`/`RouterMessage` — `dkn-protocol/src/message.rs:13,71`; `read_framed`/`write_framed`/`MAX_MESSAGE_SIZE=10MB` — `dkn-protocol/src/framing.rs`; `InferenceProof`/`TokenLogprob` — `dkn-protocol/src/proof.rs:5,20`.
- Router: `TaskRouter::dispatch_stream_internal`/`dispatch_generate` — `dkn-router/src/task.rs:570,784`; `TaskOutcome`/`TaskMeta`/`ValidationContext`/`PendingValidation` `:64,323,333,338`; `NodePool::find_nodes_for_model`/`NodeHandle` — `node_pool.rs:153,21`; `handle_node_message` — `quic_server.rs:279`; `meter_output`/`MeterReport` — `meter.rs:23,13`; `TrustScorer`/`compare_proofs`/`compute_validation_rate`/`greylist_address` — `trust.rs:242,148,77` + `task.rs:449`.