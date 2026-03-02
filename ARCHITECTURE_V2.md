# DKN v2 Architecture Plan

> A ground-up redesign of the Dria Knowledge Network for low-latency agentic inference at scale.

## Goals

1. **Single binary compute node** — no Ollama, no launcher, no `.env` juggling. Download, run, earn.
2. **Sub-second task routing** — from 14-hop batch pipeline to 4-hop direct routing.
3. **Cloud-agnostic** — no AWS vendor lock. Runs on any infrastructure.
4. **Agentic-first** — real-time inference, streaming tokens, multi-model sessions, sub-agent fan-out.
5. **Provable inference** — validation embedded in the architecture, not bolted on.
6. **Scale to millions of nodes** — stateless router fleet, horizontal scaling at every layer.

## Current State (v1) — What We're Replacing

### Problems

| Problem | Impact |
|---|---|
| 14-hop task pipeline (Client → API → S3 → PG → EventBridge → Validator → PG → Dispatcher → RabbitMQ → RPC → Dispatcher API → libp2p → Node → Ollama) | Minutes of latency per task, unusable for agents |
| Ollama as separate installation | Friction for 292K operators, HTTP overhead for local inference, no access to model internals |
| Hardcoded model enum in Rust | Every new model requires binary release across entire fleet |
| AWS-locked orchestration (ECS, EventBridge, S3, SQS) | Cannot deploy outside AWS |
| 10+ services (Harbor, Dispatcher, RPC, Challenger, NDX, Cortex, etc.) | Operational complexity, many failure points |
| Batch-only paradigm | Cannot serve agentic workloads that need real-time responses |
| RPC gateway bottleneck (star topology, single connection per node) | Single point of failure per node cluster |
| No backpressure — nodes can't reject tasks | Overloaded nodes queue indefinitely |
| Challenger uses gameable deterministic puzzles | Bad actors can pass challenges without running models |

### Current Service Count: 10+

- Harbor API, Validator, Uploader, Dashboard, Cortex, NDX, Models, Points, Status (TypeScript)
- Dispatcher (Rust)
- RPC Gateway (Rust)
- Challenger API (Python)
- Compute Node + Launcher (Rust)
- Ollama (Go)
- RabbitMQ, PostgreSQL, MongoDB, Redis, S3, SQS, EventBridge

## v2 Architecture

### Components: 3

1. **Dria Node** — single Rust binary with embedded inference (community-operated)
2. **Dria Router** — stateless routing + validation fleet (Dria-operated, horizontally scalable)
3. **Shared State** — NATS (messaging) + Redis (node registry) + PostgreSQL (persistent data)

### System Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                        Client Layer                          │
│                                                              │
│  ┌──────────┐  ┌───────────┐  ┌────────────────────────┐    │
│  │ Agent SDK│  │ Batch API │  │ Sub-agent Orchestrator │    │
│  └────┬─────┘  └─────┬─────┘  └───────────┬────────────┘    │
└───────┼───────────────┼────────────────────┼─────────────────┘
        │               │                   │
        ▼               ▼                   ▼
    (HTTPS / WebSocket / gRPC — pick per use case)
        │               │                   │
┌───────┴───────────────┴───────────────────┴──────────────────┐
│                    Load Balancer                             │
│              (nginx / envoy / any cloud LB)                  │
└───────┬───────────────┬───────────────────┬──────────────────┘
        │               │                   │
        ▼               ▼                   ▼
┌──────────────┐ ┌──────────────┐  ┌──────────────┐
│   Router A   │ │   Router B   │  │   Router N   │
│  (stateless) │ │  (stateless) │  │  (stateless) │
│              │ │              │  │              │
│  - Routing   │ │  - Routing   │  │  - Routing   │
│  - Validate  │ │  - Validate  │  │  - Validate  │
│  - Stream    │ │  - Stream    │  │  - Stream    │
└──────┬───────┘ └──────┬───────┘  └──────┬───────┘
       │                │                 │
       │    ┌───────────┴──────────┐      │
       │    ▼                      ▼      │
       │ ┌──────┐  ┌────────────────┐     │
       ├─│ NATS │  │ Node Registry  │─────┤
       │ │      │  │ (Redis/etcd)   │     │
       │ └──────┘  └────────────────┘     │
       │                                  │
       ▼     (QUIC persistent conns)      ▼
┌─────────────────────────────────────────────────────────────┐
│                   Compute Nodes (292K+)                     │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  dria-node (single Rust binary)                       │  │
│  │                                                       │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐  │  │
│  │  │  Inference   │  │   Network    │  │  Identity   │  │  │
│  │  │  (llama.cpp) │  │   (QUIC)     │  │  (secp256k1)│  │  │
│  │  └─────────────┘  └──────────────┘  └─────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Component 1: Dria Node

The compute node is a single statically-linked Rust binary that community operators download and run. It embeds the inference engine, manages models, connects to routers, and proves its work.

### Install & Run

```bash
# Install
curl -fsSL https://dria.co/install | bash

# Run (interactive first-time setup)
dria-node start

# Or fully non-interactive
dria-node start --wallet 0x... --model gemma3:12b

# Multi-model
dria-node start --wallet 0x... --model gemma3:4b,llama3.1:8b
```

No Ollama installation. No `.env` files. No launcher binary. The node downloads GGUF weights from HuggingFace on first run, validates hardware capability, connects to a router, and starts accepting work.

### Internal Architecture

```
dria-node binary (~3,000 lines Rust, single crate)
│
├── main.rs                  # CLI, startup, signal handling
├── config.rs                # Config from args/env, proper error handling (no panics)
│
├── inference/
│   ├── engine.rs            # llama.cpp bindings, load model, run inference
│   ├── models.rs            # GGUF download from HuggingFace, file management
│   ├── stream.rs            # Token-by-token streaming callback
│   └── proof.rs             # Logprob extraction, KV-cache fingerprinting
│
├── network/
│   ├── connection.rs        # QUIC connection to router, auto-reconnect
│   ├── protocol.rs          # Message types, serialization (flat, no base64)
│   └── auth.rs              # secp256k1 challenge-response handshake
│
├── worker.rs                # Task execution loop, backpressure, capacity reporting
└── identity.rs              # Wallet, keypair, address derivation
```

### Key Design Decisions

#### Models Are Strings, Not Enums

```rust
// v1 (current) — adding a model requires a release
enum Model {
    #[serde(rename = "gemma3:4b")]
    Gemma3_4b,
    // ... every model is a variant
}

// v2 — models are just identifiers
struct ModelSpec {
    name: String,           // "gemma3:12b"
    gguf_repo: String,      // "bartowski/gemma-3-12b-it-GGUF"
    gguf_file: String,      // "gemma-3-12b-it-Q4_K_M.gguf"
    chat_template: String,  // jinja2 template name or inline
}
```

The router maintains the model registry and pushes specs to nodes. Adding a new model is a config change on the router side — zero node updates needed.

#### Embedded Inference via llama.cpp

```rust
// Rust bindings to llama.cpp (via llama-cpp-2 or llama-cpp-rs crate)

pub struct InferenceEngine {
    model: LlamaModel,
    ctx: LlamaContext,
}

impl InferenceEngine {
    /// Load a GGUF model from disk
    pub fn load(model_path: &Path, gpu_layers: u32) -> Result<Self>;

    /// Run inference, streaming tokens via callback
    pub async fn generate(
        &mut self,
        prompt: &str,
        params: GenerateParams,
        on_token: impl FnMut(Token) -> ControlFlow,  // stream tokens out
    ) -> Result<InferenceResult>;

    /// Extract logprobs at specific positions (for validation)
    pub fn logprobs_at(&self, positions: &[usize]) -> Vec<TokenLogprob>;

    /// Hash KV-cache state at (layer, position) for proof-of-inference
    pub fn kv_cache_hash(&self, layer: usize, position: usize) -> [u8; 32];

    /// Benchmark: tokens per second on this hardware
    pub fn benchmark(&mut self, prompt: &str) -> TpsResult;
}
```

Hardware backend selection at build time (or runtime via feature flags):
- `--features cuda` — NVIDIA GPUs
- `--features metal` — Apple Silicon
- `--features rocm` — AMD GPUs
- `--features vulkan` — Cross-platform GPU
- Default: CPU (OpenBLAS/Accelerate)

Pre-built binaries for common combinations: `dria-node-linux-cuda`, `dria-node-macos-metal`, `dria-node-linux-cpu`.

#### Task Execution with Backpressure

```rust
pub struct Worker {
    engine: InferenceEngine,
    capacity: AtomicUsize,      // how many slots are free
    max_concurrent: usize,      // 1 for local GPU, N for multi-GPU
}

impl Worker {
    /// Returns None if at capacity (router will route elsewhere)
    pub fn try_accept(&self, task: Task) -> Option<TaskHandle> {
        if self.capacity.load(Ordering::Relaxed) == 0 {
            return None;  // REJECT — tell router to re-route
        }
        self.capacity.fetch_sub(1, Ordering::Relaxed);
        Some(self.spawn_task(task))
    }
}
```

The current system queues tasks into a 1024-size channel and hopes for the best. The new system exposes real capacity — the router never sends work to a node that can't handle it.

#### Single Tokio Runtime, Two Tasks

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args_and_env()?;  // no panics
    let engine = InferenceEngine::load(&config.model_path, config.gpu_layers)?;
    let identity = Identity::from_secret_key(&config.secret_key)?;

    let (conn, mut events) = Connection::connect(&config.router_url, &identity).await?;
    let worker = Worker::new(engine, config.max_concurrent);

    // Single select loop — no commander pattern, no inter-thread channels
    let cancellation = CancellationToken::new();
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Event::TaskRequest(task) => {
                    match worker.try_accept(task) {
                        Some(handle) => { /* task running, result streams back via conn */ }
                        None => conn.reject(task.id).await?,  // backpressure
                    }
                }
                Event::ValidationChallenge(challenge) => {
                    let proof = worker.generate_proof(&challenge)?;
                    conn.submit_proof(proof).await?;
                }
                Event::Ping => conn.pong(worker.status()).await?,
                Event::Disconnected => conn.reconnect().await?,
            },
            result = worker.next_completed() => {
                conn.send_result(result).await?;
            }
            _ = cancellation.cancelled() => break,
        }
    }
}
```

No commander pattern. No mpsc+oneshot roundtrips. No separate P2P thread. The connection and worker live in the same task, communicating directly.

---

## Component 2: Dria Router

Stateless Rust service operated by Dria. Handles task routing, node management, validation, and client-facing APIs. Horizontally scalable — add more instances behind a load balancer.

### Responsibilities

| Function | Description |
|---|---|
| **Client API** | Accept inference requests (real-time, streaming, batch) via HTTPS/WebSocket/gRPC |
| **Node Management** | Accept QUIC connections from compute nodes, track health and capacity |
| **Task Routing** | Match tasks to capable nodes based on model, capacity, latency, reputation |
| **Result Delivery** | Stream results back to clients, aggregate batch results |
| **Validation** | Issue proof-of-inference challenges, verify logprobs, detect anomalies |
| **Billing/Points** | Emit events to NATS for points calculation and billing |

### Internal Architecture

```
dria-router binary
│
├── main.rs
├── config.rs
│
├── api/
│   ├── rest.rs              # Batch API (POST /v2/infer, POST /v2/batch)
│   ├── websocket.rs         # Streaming API for agents
│   └── grpc.rs              # Optional gRPC for high-performance clients
│
├── nodes/
│   ├── registry.rs          # Read/write node state to Redis
│   ├── connection.rs        # QUIC listener, per-node connection management
│   ├── selector.rs          # Routing algorithm: model match → capacity → latency → reputation
│   └── health.rs            # Heartbeat monitoring, disconnect detection
│
├── routing/
│   ├── realtime.rs          # Single task → single node, stream result back
│   ├── batch.rs             # Fan-out tasks across nodes, aggregate results
│   └── session.rs           # Session affinity for multi-turn (optional KV-cache reuse)
│
├── validation/
│   ├── logprob.rs           # Request and verify logprobs from nodes
│   ├── timing.rs            # TPS anomaly detection
│   ├── kv_cache.rs          # KV-cache fingerprint challenges
│   └── reputation.rs        # Node reputation scoring, challenge frequency
│
└── events.rs                # Publish to NATS: points, billing, audit logs
```

### Scaling Model

Each router instance handles ~10,000 concurrent node connections (QUIC is lightweight per-connection). Scaling is linear:

| Nodes | Routers Needed | Infra |
|---|---|---|
| 50K | 5 | Small K8s cluster or 5 VMs |
| 292K | 30 | Medium cluster |
| 1M | 100 | Large cluster, any cloud or bare metal |

Routers are stateless — they can crash and restart without data loss. Nodes reconnect to any available router. All durable state lives in Redis (node registry) and NATS (event stream).

### Task Routing Algorithm

```
fn select_node(task: &Task, registry: &NodeRegistry) -> Option<NodeId> {
    registry
        .nodes_with_model(&task.model)      // 1. Must have the model
        .filter(|n| n.free_capacity > 0)     // 2. Must have free slots
        .filter(|n| n.reputation > THRESHOLD) // 3. Must not be blacklisted
        .sort_by(|a, b| {
            // 4. Prefer: lowest latency, then highest TPS, then lowest load
            a.avg_latency.cmp(&b.avg_latency)
                .then(b.tps.cmp(&a.tps))
                .then(a.load_percent.cmp(&b.load_percent))
        })
        .next()
}
```

If no node is available, the router returns a `503 Service Unavailable` with a retry-after hint. The client SDK handles retry with backoff.

### Client-Facing API

#### Real-time Inference (for agents)

```
POST /v2/infer
Content-Type: application/json
Authorization: Bearer <api-key>

{
  "model": "gemma3:12b",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What is the capital of France?"}
  ],
  "max_tokens": 256,
  "stream": true        // optional: stream tokens via SSE
}

→ 200 (streaming):
data: {"token": "The", "index": 0}
data: {"token": " capital", "index": 1}
...
data: {"done": true, "usage": {"prompt_tokens": 24, "completion_tokens": 12}}

→ 200 (non-streaming):
{
  "result": "The capital of France is Paris.",
  "model": "gemma3:12b",
  "usage": {"prompt_tokens": 24, "completion_tokens": 12},
  "node": "0xabc..."  // optional: which node served this
}
```

#### Batch Inference (for data processing)

```
POST /v2/batch
Content-Type: application/json
Authorization: Bearer <api-key>

{
  "tasks": [
    {"id": "task-001", "model": "gemma3:12b", "messages": [...]},
    {"id": "task-002", "model": "gemma3:12b", "messages": [...]},
    ...
  ],
  "webhook": "https://your-app.com/callback"  // optional
}

→ 202 Accepted:
{
  "batch_id": "batch-uuid",
  "status_url": "/v2/batch/batch-uuid"
}
```

Results stream to the webhook as they complete, or poll the status URL. No S3 upload/download cycle. For very large batches (100K+ tasks), the client can upload a JSONL file to any S3-compatible store and POST the URL.

#### Sub-agent Fan-out (for orchestrators)

```
POST /v2/fan-out
Content-Type: application/json
Authorization: Bearer <api-key>

{
  "tasks": [
    {"id": "classify", "model": "gemma3:4b", "messages": [...]},
    {"id": "reason-1", "model": "llama3.3:70b", "messages": [...]},
    {"id": "reason-2", "model": "llama3.3:70b", "messages": [...]},
    {"id": "summarize", "model": "gemma3:4b", "messages": [...],
     "depends_on": ["reason-1", "reason-2"]}
  ]
}
```

The router executes independent tasks in parallel across different nodes and respects dependency ordering. `summarize` only runs after `reason-1` and `reason-2` complete, and their outputs are injected into its context.

---

## Component 3: Shared State

### NATS (replaces EventBridge + RabbitMQ + SQS)

Single NATS cluster handles:
- **Task events**: routing notifications, completion events
- **Points/billing**: events emitted per completed task
- **Audit logs**: validation results, anomaly alerts
- **Inter-router communication**: if a node reconnects to a different router mid-task

NATS JetStream provides persistent streams where needed (billing events must not be lost). Regular NATS pub/sub for ephemeral events.

### Redis (node registry)

```
# Per-node state (SET by node via router, READ by any router)
node:{address}:models     = ["gemma3:12b", "llama3.1:8b"]
node:{address}:capacity   = { free: 1, max: 1 }
node:{address}:tps        = { "gemma3:12b": 45.2 }
node:{address}:router     = "router-a"
node:{address}:last_seen  = 1709312400
node:{address}:reputation = 0.95

# Model index (which nodes serve which model)
model:gemma3:12b:nodes    = SET of node addresses
model:llama3.1:8b:nodes   = SET of node addresses

# Blacklist
blacklist:{address}:{model} = TTL-based key
```

Redis is fast enough for the read-heavy routing queries (~100K reads/sec per instance). For >1M nodes, shard by model name.

### PostgreSQL (persistent data)

- Batch job records, file metadata
- User accounts, API keys
- Historical task results (for large batch jobs)
- Billing records

PostgreSQL is already cloud-agnostic. No changes needed from v1 except simpler schema (no file status state machine with 8 states).

---

## Validation: Proof-of-Inference

### Why Current Challenger Is Insufficient

The current system sends 5 types of deterministic puzzles (addition, leg counting, letter sums, algebra, word repeat). Problems:

1. All questions can be answered without an LLM (a calculator + regex handles 100%)
2. Only 5 question types — easy to build a specialized solver
3. Separate Python service, separate MongoDB — operational overhead
4. Challenges are infrequent and predictable

### v2 Validation: Three Layers

Validation is built into the router, not a separate service:

#### Layer 1: Timing Analysis (every task, zero cost)

Every inference result includes timing metadata from the embedded engine:
- `prompt_eval_time_ms`: how long to process the input
- `generation_time_ms`: how long to generate the output
- `tokens_per_second`: eval TPS for this specific request

The router maintains a statistical model of expected TPS per model per hardware class. Outliers are flagged:
- Too fast → likely not running the model (cached/faked responses)
- Too slow → possible CPU fallback when claiming GPU, or overloaded hardware
- Inconsistent → TPS varies wildly between similar-length prompts

Timing comes free from the embedded llama.cpp engine — no extra work on the node side. The router just needs to track distributions and flag statistical outliers.

#### Layer 2: Logprob Spot-Checks (random % of tasks, low cost)

When the router assigns a task, it can request logprobs at specific token positions:

```
Router → Node: {
  task: { ... },
  validation: {
    request_logprobs_at: [5, 12, 31]  // token positions
  }
}

Node → Router: {
  result: "The capital of France is Paris...",
  proof: {
    logprobs: [
      { position: 5, token: "capital", logprob: -0.23, top_5: [...] },
      { position: 12, token: "Paris", logprob: -0.08, top_5: [...] },
      { position: 31, token: ".", logprob: -1.42, top_5: [...] }
    ]
  }
}
```

The router validates by:
1. Checking logprob distributions are plausible for the model
2. Periodically cross-referencing with a trusted validator node running the same model
3. Building a per-node profile — consistent logprob patterns indicate legitimate inference

Faking logprobs requires actually running the model. They cannot be derived from the text output alone.

#### Layer 3: KV-Cache Fingerprinting (periodic challenges, highest strength)

The strongest proof: request the SHA-256 hash of the KV-cache tensor at a specific (layer, position) during inference:

```
Router → Node: {
  challenge: {
    prompt: "The quick brown fox...",
    request_kv_hash: { layer: 8, position: 15 }
  }
}

Node → Router: {
  kv_hash: "a3f8b2c1d4e5..."
}
```

This hash is deterministic for a given model + input + position. Only a node with the model loaded and actively processing the input can produce it. The router verifies against a trusted reference. This is computationally impossible to fake without running the actual model weights.

KV-cache proofs are the most expensive to verify (router needs a reference node to compare against) so they're issued periodically — more frequently for new/low-reputation nodes, less for established ones.

#### Reputation Score

Each node maintains a reputation score (0.0 to 1.0) based on:
- Validation pass rate
- Task completion rate
- Response time consistency
- Historical behavior

New nodes start at 0.5 and must build reputation through successful validated tasks. Reputation decays slowly over time (must stay active). Nodes below 0.3 are blacklisted.

```
reputation_update(node, event):
  match event:
    TaskCompleted     → +0.001
    ValidationPassed  → +0.005
    ValidationFailed  → -0.1    // harsh penalty
    TimingAnomaly     → -0.05
    Timeout           → -0.02
    Rejection         → no change (backpressure is fine)
```

High-reputation nodes get:
- Less frequent validation (cost savings for the network)
- Priority in task routing (rewarding good behavior)
- Higher point earnings multiplier

---

## Network Protocol

### Node ↔ Router: QUIC

QUIC provides:
- **Built-in encryption** (TLS 1.3) — no need for libp2p's Noise layer
- **Multiplexed streams** — no need for Yamux
- **NAT-friendly** (UDP-based) — works behind most consumer routers
- **0-RTT reconnect** — near-instant reconnection after brief disconnects
- **Connection migration** — survives IP address changes (mobile, DHCP renewal)

Rust implementation: `quinn` crate (mature, production-ready).

### Authentication Handshake

```
1. Node opens QUIC connection to Router
2. Router sends: { challenge: random_32_bytes }
3. Node signs challenge with secp256k1 private key
4. Node sends: {
     address: "0xabc...",
     peer_id: "16Uiu2HAm...",
     signature: "0x...",
     recovery_id: 0,
     models: ["gemma3:12b"],
     tps: { "gemma3:12b": 45.2 },
     version: "2.0.0",
     capacity: { free: 1, max: 1 }
   }
5. Router recovers public key from signature, verifies address
6. Router sends: { authenticated: true, node_id: "..." }
```

No persistent identity storage needed. The node proves identity on every connection using its wallet key.

### Message Format

```rust
// Flat, simple, no base64 wrapping, no nested JSON-in-JSON
#[derive(Serialize, Deserialize)]
enum NodeMessage {
    // Node → Router
    TaskResult {
        task_id: Uuid,
        result: String,
        proof: Option<InferenceProof>,
        stats: TaskStats,
    },
    TaskRejected {
        task_id: Uuid,
        reason: RejectReason,  // AtCapacity, ModelUnloaded, etc.
    },
    StatusUpdate {
        capacity: Capacity,
        models_loaded: Vec<String>,
    },
    ChallengeResponse {
        challenge_id: Uuid,
        proof: InferenceProof,
    },
}

#[derive(Serialize, Deserialize)]
enum RouterMessage {
    // Router → Node
    TaskAssignment {
        task_id: Uuid,
        model: String,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        validation: Option<ValidationRequest>,
    },
    Challenge {
        challenge_id: Uuid,
        prompt: String,
        proof_request: ProofRequest,
    },
    Ping,
    ModelRegistryUpdate {
        models: Vec<ModelSpec>,
    },
}
```

Serialized as MessagePack (binary, ~30% smaller than JSON, faster to parse) over QUIC streams. No base64 encoding. No JSON-in-JSON. No triple serialization.

---

## Migration Path

### Phase 1: New Node Binary (weeks 1-4)

Build the new `dria-node` with:
- [ ] Embedded llama.cpp via `llama-cpp-2` crate
- [ ] GGUF model download from HuggingFace
- [ ] Hardware detection and TPS benchmarking
- [ ] QUIC connection to router (using `quinn`)
- [ ] secp256k1 authentication handshake
- [ ] Task execution with streaming
- [ ] Backpressure (reject when at capacity)
- [ ] Logprob extraction for validation
- [ ] Single-binary builds for Linux (CUDA, CPU), macOS (Metal)

**Backwards compatibility**: The new node can initially speak the v1 libp2p protocol to connect to existing RPC gateways. This allows incremental rollout — operators upgrade their node binary while the backend remains unchanged.

### Phase 2: Router MVP (weeks 3-6)

Build the first Dria Router with:
- [ ] QUIC listener for node connections
- [ ] Node registry in Redis
- [ ] Real-time inference API (POST /v2/infer)
- [ ] Task routing algorithm (model match → capacity → latency)
- [ ] Result streaming via SSE/WebSocket
- [ ] Basic validation (timing analysis + logprob spot-checks)
- [ ] NATS integration for events

Run alongside v1 infrastructure. Clients can use either the v1 batch API or the v2 real-time API.

### Phase 3: Batch & Fan-out (weeks 5-8)

- [ ] Batch API (POST /v2/batch)
- [ ] Sub-agent fan-out with dependency DAGs
- [ ] Webhook result delivery
- [ ] Large batch support (JSONL upload to S3-compatible store)
- [ ] Cross-verification validation
- [ ] Reputation system

### Phase 4: v1 Deprecation (weeks 8-12)

- [ ] Migrate all batch API clients to v2
- [ ] Shut down Harbor services one by one
- [ ] Remove Dispatcher, RPC Gateway, RabbitMQ
- [ ] Remove Challenger API (validation is in-router now)
- [ ] Remove Ollama dependency from node documentation

### Phase 5: Advanced Features (ongoing)

- [ ] KV-cache proof-of-inference
- [ ] Session affinity (multi-turn KV-cache reuse)
- [ ] Model hot-swap (switch models without restart)
- [ ] Multi-GPU inference (tensor parallelism via llama.cpp)
- [ ] gRPC API for high-performance clients
- [ ] Geographic routing (prefer nodes close to the client)
- [ ] Node-to-node communication for collaborative inference

---

## Comparison: v1 vs v2

| Dimension | v1 (Current) | v2 (Proposed) |
|---|---|---|
| **Task latency** | Minutes (batch pipeline) | Seconds (direct routing) |
| **Streaming** | No | Yes (token-by-token) |
| **Node setup** | Install Ollama + Launcher + configure .env | Single binary, one command |
| **Adding a model** | Code change + binary release | Config update on router |
| **Services to operate** | 10+ | 3 (Router, NATS, Redis) |
| **Cloud dependency** | AWS (ECS, EventBridge, S3, SQS) | Any cloud or bare metal |
| **Validation** | Gameable math puzzles | Logprobs, timing, KV-cache proofs |
| **Backpressure** | None (queue and hope) | Nodes reject, router re-routes |
| **Agentic support** | None (batch only) | Real-time, streaming, fan-out, DAGs |
| **P2P protocol** | libp2p (TCP+Noise+Yamux+CBOR) | QUIC (built-in encryption+mux) |
| **Inference engine** | Ollama (separate process, HTTP) | Embedded llama.cpp (in-process) |
| **Node count scaling** | ~30 RPC gateways | Stateless router fleet, linear scaling |
| **Code size (node)** | ~5,300 lines, 4 crates | ~3,000 lines, 1 crate |
| **Message format** | JSON → base64 → sign → JSON → CBOR | MessagePack, signed, direct |

---

## Open Questions

1. **QUIC vs WebSocket**: QUIC is technically superior but WebSocket has broader NAT traversal success. Could offer both — QUIC primary, WebSocket fallback for restrictive networks.

2. **llama-cpp-2 vs candle**: llama.cpp has the best hardware coverage and GGUF ecosystem. Candle is pure Rust (simpler builds) but narrower model support. Recommend llama.cpp for now.

3. **Model registry governance**: Who decides which models are available on the network? Currently hardcoded in Rust. Should be a curated list managed by Dria, pushed to nodes via router.

4. **Multi-GPU nodes**: Some operators have 8x H100 setups. The new node should support tensor parallelism via llama.cpp's built-in support. How does this affect task routing?

5. **Pricing model**: v1 is per-token. v2 could be per-token (same), per-request (simpler), or per-second-of-compute (fairer for different model sizes).

6. **SDK design**: The agent SDK (Python/TypeScript) should abstract away routing, streaming, retries. What does the ideal developer experience look like?

7. **Testnet**: Replace mockollama with a `--mock` flag built into the node binary. The node generates deterministic responses without loading a real model. Simpler than a separate mock server.
