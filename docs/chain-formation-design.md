# DKN Pipeline Chain Formation — Definitive Design

**Status:** control-plane design for node↔node pipeline-parallel serving. Sits on top of the runtime in `docs/pipeline-specdecode-design.md` (graph cut, async spec-decode, `KvTruncate`) — does not re-specify those. Anchored to real `dkn-router` symbols verified in source.

---

## 1. The reframing — chains are pre-warmed standing assets, not per-request artifacts

**The single inversion that drives everything:** decouple the chain from the request. Today the unit of provisioning is a *task* dispatched to one node that holds nothing between tasks (`find_nodes_for_model` → shuffle → `decrement_free` → dispatch). For pipelines that model is fatal: assembling a chain on demand means cold-loading layer blocks (minutes of GGUF load) and cold-punching NATs (seconds) on the request path. That is categorically incompatible with the ms-bind requirement.

So the unit of provisioning becomes a **`WarmChain`** (`= WarmPath`): a standing, named, epoched ring that exists in router memory and in nodes' VRAM **before any user asks**, and **survives across many requests**. Requests *rent* a warm chain; they never *create* one.

**`Warm` is a four-part predicate — all must hold before a chain enters the bind index:**

1. **Layers resident.** Every shard called `load_shard(layer_range)` (Phase-1 runtime primitive); its contiguous layer block is in VRAM. Expensive, slow, done **once at build, never per request.**
2. **Edges tested.** Every consecutive pair `(i, i+1)` plus `tail→0` (direct return) completed NAT traversal and holds an **idle keepalive QUIC stream** (QUIC PING ~10–15 s keeps the NAT binding alive, so binding needs zero punch).
3. **Ring ordered for min latency.** Node order is the Held-Karp min-RTT Hamiltonian path over the chosen shards (n≤16 exact, ported from shard `topology.py`), with the `tail→0` return edge in the cost. Node-0 pinned to start by role.
4. **Pipeline self-tested.** Before serving, the chain runs one **synthetic warm-up generation** (fixed dummy prompt) end-to-end: validates the graph cut reproduces correct logits, measures real `l_traversal` and **per-stage compute time**, confirms KV rollback. Fail → `Quarantined`, placer rebuilds.

All four are produced by a **continuous background path-maintenance loop on the router**. Nothing on the request path may cold-load, cold-punch, or run the placer. We pay for this with idle VRAM (warm chains + standbys holding weights for traffic that may not come); that cost is named explicitly in §8 and is the deliberate currency we spend for bind-time determinism.

---

## 2. Node capability registration

Capability must be captured at the one place the router already learns about a node — and the full network address, which is **currently thrown away**, must be retained.

**Ride point — `AuthRequest` (handshake, `quic_server.rs::handle_connection`).** The handshake already carries `address`, `models: Vec<String>`, `tps: HashMap<String,f64>`, `capacity: Capacity`. Extend `AuthRequest` (in `dkn-protocol`) with a `ShardCapability`:

```rust
struct ShardCapability {
    vram_free_gb: f32,
    vram_total_gb: f32,
    gpu_kind: String,              // "rtx4090" | "a6000" | "mi300" | "m3max" ...
    supports_layer_split: bool,    // node built with the shard runtime
    reachable: Reachability,       // PublicIp | Punchable | RelayOnly (node's self-probe)
    geo_hint: Option<String>,      // optional coarse self-reported region
    // measured local tok/s already exists as auth_req.tps (today #[allow(dead_code)] — now read)
}
```

**Critical capture fix (verified gap):** `quic_server.rs:151` reads `remote = conn.remote_address()` but line 211 collapses it to a /24-or-/48 subnet string and discards the rest. Add `pool.set_socket_addr(&node_id, remote)` **beside** the existing `pool.set_subnet(...)` at line 211. The full `SocketAddr` is the seed for IP-geolocation and the prober.

**`NodeHandle` extensions** (`node_pool.rs:21`, today has only `node_id, address, models, tps, capacity, subnet, sender`):

```rust
socket_addr: Option<SocketAddr>,   // from conn.remote_address() — NEW capture
vram_free_gb: f32,
gpu_kind: String,
reachable: Reachability,
geo_cell: GeoCell,                 // (continent, country, asn_group) from IP geolocation
throughput_est: PerLayerMs,        // measured at warm-up self-test (§1.4), NOT self-reported
cluster_ids: SmallVec<[ClusterId; 2]>,
last_heartbeat: Instant,           // NEW — no last-seen exists today (liveness == live QUIC conn)
rtt_to_router_ms: Option<f32>,     // from pong on the 30s ping loop
```

**Heartbeat ride point.** No separate heartbeat exists; `NodeMessage::StatusUpdate { models, capacity, version, stats }` (`quic_server.rs:327`) already arrives, but `version` and `stats` are discarded (`_`) and only `update_status(node_id, models, capacity)` is applied. Piggyback refreshed `ShardCapability` (vram drift, reachability) onto `StatusUpdate` at the existing ~10 s inventory cadence; extend `update_status` to apply it and stamp `last_heartbeat`. **3 missed heartbeats (30 s) → removed from all candidate sets and triggers standby promotion if it was serving.**

**`ALLOWED_MODELS` (verified gap, `quic_server.rs:13`):** a hardcoded compile-time `LazyLock<HashSet<&'static str>>`; `filter_models` silently drops anything not in it. Chain-capable model variants **must** be added here or the pool never sees them. Treat each layer-cut-plan version as a distinct model id (so validation/standbys never pick a node loaded with the wrong cut).

**Redis (new keys, follow the `inventory.rs::start_inventory_publisher` set_ex pattern; today no key models VRAM/RTT/topology):**

| Key | Type | TTL | Writer |
|---|---|---|---|
| `node:{addr}:capability` | JSON `ShardCapability` | 60s | heartbeat handler |
| `node:{addr}:rtt` (router) | float | 60s | ping/pong loop |
| `node:{addr}:peer_rtt` | hash `peer→EdgeStat` | 300s decay | prober |
| `chain:{chain_id}` | JSON `WarmChain` snapshot | 30s | path-maintenance loop |
| `path:{router_id}:{model}` | set of `chain_id` | 30s | path-maintenance loop |

---

## 3. Connectivity & RTT probing

Full mesh is `O(n²)` — at 20k that is 4×10⁸ probes and a 20000² matrix per re-measure. **Never measure the full mesh.** Three loops at three cadences; fast where it must be fresh, sparse where it is stable.

**(a) Router↔node RTT — every 30 s, free.** Reuse the existing ping loop (`quic_server.rs:64-70`, `pool.broadcast(|| RouterMessage::Ping)`). Pair `Ping` with a timestamp; the node replies via `NodeMessage::ChallengeResponse` — **currently received and ignored at `quic_server.rs:335`, a free slot** — carrying the echoed timestamp. Record `rtt_to_router_ms`. This gives the solver's `c_out` (router→head) and `c_in` (tail→router) **for free**, for every node, with zero new transport.

**(b) Candidate-edge probing — geo-pruned, sampled, ~minutes refresh.** For *building* chains the router needs node↔node RTT, but only within latency-coherent neighborhoods. Derive `geo_cell = (continent, country, asn_group)` per node from `socket_addr` via MaxMind/IP2Location at connect + on IP change (the *prior*; cheap, never per-request). Prune the probe matrix to **intra-cell + adjacent-cell** pairs only. Each node probes a **rotating sample** of those peers every ~60 s (router-coordinated via a punch broker; the punch itself is the reachability ground-truth). Measure, over the **same QUIC link the pipeline will hold open**, with `TCP_NODELAY`-equivalent (disable batching), 3 warmup probes discarded, **median of ~20 small payloads** (lift shard `mesh.py::rtt_to` discipline — median, not min/mean; not ICMP):

- `rtt_p50, rtt_p95` (asymmetric, **never symmetrized** — `L[a][b] ≠ L[b][a]` allowed)
- loss / reordering under QUIC (nonzero ⇒ bad path)
- sustained throughput at the **real hidden-state frame size** (`[γ+1, H]` bf16, ~12 KiB) — a thin home uplink that can't sustain the per-token frame rate fails *here*, before it ever serves
- `punch_ok` (reachability truth, can differ from self-reported `reachable`)

Stored in `LatencyCluster.rtt_matrix` (sparse) and Redis `node:{addr}:peer_rtt`, **TTL 300 s with decay** so flaky edges age out.

**(c) Warm-chain internal-link health — fast, 2–5 s, scoped.** The links *inside* a warm chain are latency-critical and must be known-good *before* a request binds. Each warm chain probes its own consecutive edges + `tail→0` every 2–5 s via the keepalive stream (the QUIC PING doubles as the RTT sample) — a `ShardPing{chain_id, epoch, seq}` / `ShardPong` over the **data edge**, not the router link (the router↔node QUIC being alive does not prove node-i↔node-(i+1) is alive — different NAT binding). A chain whose `l_traversal` drifts above ceiling, or that drops a link, is **demoted out of the bind index immediately** (no new request binds) and handed to the placer for repair. **This is affordable at scale precisely because the fast probe is scoped to the few hundred links actually in warm chains, not the whole mesh.** Apply hysteresis (sustained failure, not one missed pong) so a 2 s home-WiFi blip does not trigger a promotion storm (§8).

**Geo doctrine: RTT is ground truth, geo is only the prior.** Same-city/different-ISP can be worse than cross-city/same-backbone (internet RTT tracks peering, not distance). Geo-IP prunes the probe set; the measured matrix decides clusters and ordering. A mis-clustered node (VPN/mobile/bad GeoIP) is corrected by the next refold — but can drag a chain until then (§8).

---

## 4. Chain assembly algorithm

Runs **only in the path-maintenance loop**, never in dispatch. Chains are built *within* a `LatencyCluster` (a set whose measured intra-RTT < `cluster_rtt_ceiling`, start 40 ms p50; size 30–150 nodes, seeded by `geo_cell`, refined by probes; clusters overlap at boundaries so a node can be a candidate in two). **Do not build cross-continent chains** — an ocean hop is paid on *every* traversal forever; serve cross-region demand by **replicating warm chains per region**, not one global ring.

**Step 1 — N from the recruited VRAM tier.** `N = ceil(W / (V × util))`, `util ≈ 0.7` (room for KV + activations + standby headroom), `V` = modal free-VRAM of the cluster's candidate set. **Clamp `N ∈ [6, 10]` as the design point, hard ceiling 16** (Held-Karp exact limit + the WAN latency wall: per-token cost is a full ring traversal). If even N=16 won't fit the tier, the model is **not servable** on that tier — refuse, don't build a 20-deep chain.

| Tier | VRAM/node | layers/node (78-layer GLM) | N |
|---|---|---|---|
| A (48 GB+: A6000/6000-Ada) | ~38–63 GB usable | 10–13 | **6–8** ← design point |
| B (24 GB: 3090/4090) | ~19 GB | 5–7 | **11–14** |
| C (≤16 GB) | ~12 GB | 3–4 | up to 16 (last resort) |

**Step 2 — assign layers to balance STAGE COMPUTE TIME, not layer count or VRAM.** The assembly-line bottleneck is the *slowest stage*. Naive memory-weighted partition (exo's `fraction ∝ mem`) is **wrong**: a 48 GB slow Ampere and a 24 GB fast Ada should get counts that equalize `layers_i × per_layer_ms_i`, where `per_layer_ms_i` comes from the warm-up self-test (§1.4), not from VRAM. Placer solves:

```
minimize   max_i ( layers_i · per_layer_ms_i )           # flatten the bottleneck stage
s.t.       layers_i · bytes_per_layer ≤ vram_free_i · util   # VRAM = constraint
           Σ layers_i = L,  contiguous blocks
```

Greedy water-filling suffices (assign layers to the fastest node with headroom until its stage time catches the next). A slow 48 GB node may end up with *fewer* layers than a fast 24 GB node — correct. **Prefer homogeneous-tier chains**; mixing forces stage-time imbalance and idles every other node. The block boundaries are recorded as the model's **canonical `LayerCutPlan`** so standbys are fungible (§5).

**Step 3 — order the ring by measured RTT (Held-Karp).** Lift shard `topology.py::_held_karp` (~60 lines, Apache-2.0, attribute) + `loop_cost` + `_nn_2opt` fallback (for n>16) verbatim. Objective: `c_out[head] + Σ L[i][i+1] + c_in[tail]`, asymmetric, never symmetrized. The router **is** the depot/coordinator and already has `c_out`/`c_in` from §3a. Node-0 pinned to start by role; the solver orders the middle and chooses head/tail. Re-solve on any re-plan.

**Step 4 — reachability constrains *position*, not just membership.** A **middle hop needs inbound reachability** (receives a connection from its predecessor). Endpoints have slack: node-0 talks out to head and receives the tail's direct return; tail only receives + returns to node-0. So:
- `RelayOnly` / hard-symmetric-NAT nodes → **deep middle only** (and only with a relay in budget) — never node-0 or tail, where a drop is most disruptive and re-punch hardest.
- `PublicIp` / `Punchable` → eligible for any seat, **preferred for node-0 and tail**.

**Step 5 — activation-privacy boundary pinning.** A malicious mid-pipeline node can reconstruct 35–59% of user tokens from activations. **Pin the embedding block (node-0) and the final block (tail) to staked/trusted nodes** (fold the existing `TrustScorer` honesty score into seat eligibility); leave only deep-middle blocks to untrusted volunteers. This is a product/security decision, enforced at placement.

---

## 5. Warm-chain pooling & lifecycle

**How many warm chains per (model, cluster) — demand-scaled with a floor.** Maintain `desired_warm_count = ceil(EWMA_rps × p99_request_secs / chain_capacity) + 1`, with a **floor of 1 per model per cluster that has any traffic**. A chain hosts many concurrent requests (async pipeline interleaves verify-chunks across requests; each request = own `request_id` + own KV sequence per shard); capacity = `free_kv_slots = min over shards`. So `desired_warm_count` is single-digit even for popular models. The `+1` headroom buys instant burst absorption.

**Three-pool model (per model):**

| Pool | Holds | VRAM | Count |
|---|---|---|---|
| Eligible | nothing (advertises capability) | 0 | the qualifying 20k slice |
| Warm (in-path) | its block, links tested, in a `Ready` chain | full shard | `N × desired_warm_count` |
| Hot-standby | **same block as some primary**, KV-empty, links to that primary's neighbors **pre-probed** | full shard | `R × N × small const` |

**Standby fungibility — the biggest VRAM lever.** Because all warm chains of a model share the **canonical `LayerCutPlan`**, one standby holding block `[a,b)` defends *every* chain that uses that cut. Standby VRAM collapses from `R×N×desired_warm_count` to `R×N×(small const)`. A standby's warm-time work = load block + **pre-probe links to the i-1 and i+1 of the primaries it can replace** (so promotion is a metadata flip, not a cold punch).

**Churn-scaled redundancy R** (don't double-pay stable seats): `PublicIp`/datacenter (~0.5%/hr churn) → **R=1**; `Punchable` consumer (~3%/hr) → **R=2**; `RelayOnly`/residential (~8%/hr) → **R=2 and never seat as node-0/tail**.

**Lifecycle state machine** (router path-maintenance reconciliation loop, ~1–5 s tick — a controller converging warm capacity toward measured demand and churn):

```
provision → Warming ──(all ShardReady + all edges probed-OK + self-test pass)──► Ready ──bind requests──┐
               │ (probe/load/self-test fail)                                       │                     │
               ▼                                                                    │                     │
            Quarantined→rebuild                                                     │                     │
Ready ──primary drop, standby avail──► [promote, epoch++, re-prefill] ──► Ready     │                     │
Ready ──primary drop, NO standby─────► Degraded ──cold-recruit OK──► Ready          │                     │
                                          └─(deadline ~2s)─► drain in-flight to other Ready chain ─► Discard
Ready ──EWMA below target / LRU idle 10min above floor──► Draining ──► Discard (free VRAM)
Ready ──LayerCutPlan version bump──────► Draining ──► Discard (replaced by new-plan chain)
```

**Request binding — the millisecond path (§6 detail):** registry lookup of `Ready` chains for M, pop lowest `l_traversal` with a free slot, stamp `request_id`+`plan_epoch`, `get_sender(node0).send(prompt)`. No load, no punch, no solve.

**Healing on churn (`plan_epoch` + re-prefill — the load-bearing reliability mechanism):**
- Every `WarmChain` carries a monotonic `epoch`; every data frame carries `(chain_id, epoch)`. **A node drops any frame whose epoch < its current.** This fences stale in-flight frames addressed to a dead primary — promotion is atomic from the data plane's view, no distributed agreement (node-0 is the single writer of the committed sequence).
- **Promote, don't recruit (target <300 ms):** router picks the healthiest pre-probed hot-standby for block-i, issues `PromoteStandby{epoch+1}`; i-1's `next_hop` and i+1's `prev_hop` flip to the standby (metadata flip — links pre-punched at warm-time; stale-binding fallback = fast re-punch); epoch bumps; dead primary's frames fenced out.
- **Re-prefill, don't migrate KV (KV never crosses the wire):** the promoted standby has layers but no KV. Node-0 holds the **canonical committed-token prefix** per request (it must, for spec-decode accept/reject). On promotion: broadcast `KvTruncate{keep_to_pos: n_committed}` (reuse the spec-decode rollback op) to **all** surviving shards (snaps off interrupted speculative positions), then re-prefill the committed prefix through the chain in **one batched pass** so only the new shard rebuilds its KV slice. Loop resumes at `epoch+1` from `n_committed`. **The request resumes, never restarts; greedy output stays bit-identical.**
- **Standby exhaustion → `Degraded`:** cold-recruit that one block from Eligible (slow, seconds — layer load); in-flight requests that can't recover within ~2 s are re-dispatched to another `Ready` chain (node-0 re-prefills the committed prefix there). Replenish the drained standby pool async.

---

## 6. Router + protocol changes (grounded in verified symbols)

**New module `dkn-router/src/pipeline.rs`** — hosts the placer (Held-Karp + water-fill layer assignment), the path-maintenance loop, the cluster-maintenance loop, and the structures: `LatencyCluster`, `WarmChain`, `ShardSlot{node, layer_range, role, standby}`, `BindIndex` (per-model min-heap by `(has_free_slot, l_traversal_p50, load)` — the **only** hot-path structure), `WarmTarget{ewma_rps, desired/current_warm_count}`, `LayerCutPlan`.

**`ChainRouter` modeled on `TaskRouter` (`task.rs:349`)** — mirror its `pending: DashMap<Uuid,…>` with `pending_chains: DashMap<ChainId, WarmChain>`. Add `PendingChainGuard` modeled on `PendingTaskGuard` (`task.rs:32-60`) but reserving/releasing a slot on **all N hops atomically** — verified gap: today `decrement_free`/`increment_free` (`task.rs:709`, `node_pool.rs:197-204`) are single-slot; a chain build must `decrement_free` all N node_ids and roll back **all** on any failure.

**`NodePool` (`node_pool.rs`)** — add `set_socket_addr`, `find_path_for_model(model, cluster, n_shards) -> Option<Vec<String>>` (filters `model_index`, requires `capacity.free>0` on all hops, orders via the placer; **replaces the unordered-shuffle `find_nodes_for_model` for chains** — verified: that method's doc comment claims "sorted by free capacity" but it just `shuffle`s). `find_nodes_for_model` stays for single-node legacy dispatch.

**`dispatch_pipeline` = the §6 bind** (new in `task.rs`, parallel to `dispatch_generate`). `TaskOutcome::Failed{error}` already exists (`task.rs:64`) and is treated as retry — reuse it so a bind to a momentarily-bad chain retries the next warm chain.

**New protocol types (`dkn-protocol`):**
- `RouterMessage`: `PipelineAssignment{chain_id, epoch, layer_range, prev_hop, next_hop, owner_addr, role}`, `PromoteStandby{chain_id, epoch, slot}`, `ProbeCoordinate{peer, candidates, punch_at, role}`, `KvTruncate{request_id, epoch, keep_to_pos}` (reused from runtime doc), `PipelineTeardown{chain_id}`.
- `NodeMessage`: `ProbeResult{peer, EdgeHealth{rtt_p50, rtt_p95, loss, mbps, punch_ok}}`, `ShardReady{chain_id, layer_range}`, `PeerRtt{...}`, pong data on the existing `ChallengeResponse` slot (`quic_server.rs:335`).
- `ShardMessage` (node↔node data plane, settled in runtime doc): `HiddenStateFrame`, `ShardPing/ShardPong`, `VerifyResult` — each carries `(chain_id, epoch)`.
- Match all new `NodeMessage`/`RouterMessage` variants in `quic_server.rs::handle_node_message` (`match` at line 286/279).

**NAT punch coordination** — router brokers `ProbeCoordinate` (simultaneous QUIC open between two nodes) at **warm time**, proactively, reusing the punch machinery; `RelayOnly` pairs fall back to a router/relay path. The full `socket_addr` captured in §2 is what makes the broker possible.

**Prober** — periodic `tokio::spawn` started in `start_quic_server`, mirroring `inventory.rs::start_inventory_publisher`; runs loops (a)/(b)/(c) from §3, persists to the Redis keys in §2.

**Config knobs (`config.rs`, flat clap `Cli`/`Config` + `from_cli` mirror + update every `Cli{..}` test fixture, lines ~173-294):** `--chain-enabled`, `--chain-max-shards` (16), `--chain-target-n` (8), `--cluster-rtt-ceiling-ms` (40), `--probe-interval-secs`, `--warm-idle-ttl-secs` (600), `--warm-headroom` (1), `--standby-redundancy-public/punchable/relay`, `--pin-endpoints-to-staked` (bool).

---

## 7. Phased rollout (each independently testable)

1. **One static hand-pinned chain.** Hardcode N node_ids + even contiguous layer split (shard `assign_layers`); `--next` wiring via `PipelineAssignment`; dispatch one request through it. Proves the hidden-state cut + ring + `tail→0` return + node-0 spec-decode loop. *Test: greedy output bit-identical to single-node baseline; measure `l_traversal`.*
2. **Capability registration + router↔node RTT.** `ShardCapability` on `AuthRequest`, `socket_addr` capture, pong on the ping loop, `geo_cell`. *Test: Redis `node:{addr}:capability`/`:rtt` populated; handle carries vram + addr.*
3. **Measured-RTT ordering.** Candidate-edge prober (§3b) + Held-Karp + `loop_cost`. *Test: solver picks the min-`loop_cost` order on a synthetic RTT matrix; matches a brute-force check for n≤8.*
4. **Compute-balanced layer assignment.** Warm-up self-test measures `per_layer_ms`; water-fill partition. *Test: heterogeneous 2-tier cluster → stage times within X% of each other; throughput = slowest stage.*
5. **Warm pools + ms-bind.** `WarmChain` lifecycle, `BindIndex`, demand-scaled `desired_warm_count`, multi-hop reservation guard. *Test: warm chain reaches `Ready`; bind latency < 5 ms (lookup + send, no load/punch); LRU eviction above floor.*
6. **Standby healing.** Hot-standbys (fungible, churn-scaled R), pre-probed links, `PromoteStandby` + `epoch` fencing + `KvTruncate` re-prefill. *Test: kill a middle shard mid-generation → promotion <300 ms, request resumes, output unchanged.*
7. **Demand scaling + cluster refold.** EWMA autoscaler with hysteresis, cluster-maintenance loop, cross-cluster fallback. *Test: 10× synthetic spike → scale-up without oscillation; cluster refold corrects a mis-GeoIP'd node.*

---

## 8. Hard tradeoffs / open knobs

- **Pre-warm VRAM cost vs bind latency** — the defining trade. Warm chains + the `+1` headroom + churn-scaled standbys hold weights for traffic that may not arrive. Idle VRAM is the currency we *choose* to spend to buy ms-bind determinism. Knob: `--warm-headroom`, per-model floor. **Bias up** — one extra idle warm chain is cheaper (in latency + reputation) than ever making a request wait on a minutes-long cold build. Mitigate with **standby fungibility** (shared `LayerCutPlan`) — the single biggest reducer — and price standbys explicitly as an insurance retainer or nobody volunteers.
- **Pool size vs churn** — small N means each shard is more load-bearing (a Tier-A 13-layer hole is far bigger than a Tier-C 3-layer one) and homogeneous-tier preference shrinks the substitution set. Knob: `--standby-redundancy-*` per tier. Concentrate redundancy where drops are frequent, minimize it on stable public-IP seats.
- **Probe overhead vs freshness** — fast probing everywhere is infeasible at 20k. Resolution: **fast (2–5 s) only on warm-chain internal links** (few hundred), **sparse (minutes, geo-pruned, sampled) for candidate edges**, **free (30 s) for router↔node**. The residual risk is **stale-warmth**: a candidate edge that *was* good can be rotten when the placer finally builds on it (caught at warm-up self-test → wasted build, not a bad serve).
- **N under mixed VRAM tiers** — prefer homogeneous-tier chains per cluster; when forced to mix, set N by the *constraining* (smallest-VRAM) tier and **balance stage *time*** via water-fill (`fraction ∝ throughput`, VRAM as constraint), so the slow node gets fewer layers. Never set N or layer counts proportional to VRAM alone — that re-introduces the bottleneck the whole assembly line is gated on.
- **Two failure modes to instrument from day one** (both are direct consequences of latency-first + reliability-first biases): **(1) hot-spotting** — the bind index always pops the best chain and the placer always prefers Tier-A public-IP nodes, so a few excellent nodes get recruited into *every* chain and saturate (concentrates earnings + trust risk + thundering-herd). Mitigate: per-node chain-membership cap + a mild load term in the bind key + spread `desired_warm_count` across clusters. **(2) Promotion-storm thrashing** — eager heartbeat thresholds promote on transient blips, burning standbys right before a real failure. Mitigate: hysteresis (sustained failure, not one missed pong), exponential backoff on re-promoting a slot, per-chain promotion-rate cap that trips to `Degraded`+rebuild rather than looping.

---

**Anchor files:** `dkn-router/src/node_pool.rs` (`NodeHandle:21`, `NodePool:51`, `find_nodes_for_model:153`, `subnet_key:37`, `decrement_free:197`) · `dkn-router/src/quic_server.rs` (`ALLOWED_MODELS:13`, `handle_connection:144`, `remote_address:151`, ping loop `:64-70`, `set_subnet:211`, `handle_node_message:279`, `StatusUpdate:327`, free `ChallengeResponse` slot `:335`) · `dkn-router/src/task.rs` (`TaskRouter:349`, `PendingTaskGuard:32-60`, `decrement_free:709`, `TaskOutcome::Failed:64`) · `dkn-router/src/inventory.rs` (`start_inventory_publisher:109`, TTL/interval consts `:12-13`) · `dkn-router/src/trust.rs` (`TrustScorer` for endpoint-pinning + trust-weighted edges) · `dkn-router/src/config.rs` (flat clap knobs) · new `dkn-router/src/pipeline.rs` · runtime counterpart `dkn-compute-node/docs/pipeline-specdecode-design.md` · solver to port `/tmp/leyten-shard/shard/topology.py`.