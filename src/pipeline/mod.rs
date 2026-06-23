//! Pipeline-parallel GLM-5.2 serving (all-Luminal).
//!
//! This is the orchestration core of the `luminal-glm-pipeline` fork: shard the GLM-5.2
//! layers across a node chain, pass residual-stream hidden states node→node over QUIC, and
//! drive generation with node-0-local speculative decoding. The compute substrate is
//! Luminal (CUDA/NVFP4 frontier lane); see the design docs:
//!   - `docs/substrate-decision.md`         — why Luminal, why all-Luminal
//!   - `docs/pipeline-specdecode-design.md` — the runtime (cut, async spec-decode, rollback)
//!   - `docs/chain-formation-design.md`     — the control plane (warm chains, prober, placer)
//!
//! Layering: [`engine`] defines substrate-agnostic traits; [`specdecode`] is the driver
//! written purely against them (and is fully unit-tested without a GPU); [`transport`] is
//! the node↔node hidden-state link (stub, to be implemented over the existing quinn stack).
//!
//! Migration note: the existing llama.cpp path (`crate::inference`, `crate::models`) is
//! retained for now to keep the build green; it is removed as the Luminal engine lands
//! (the substrate decision is all-Luminal, GLM-5.2-only).

pub mod engine;
pub mod specdecode;
pub mod specpipe;
pub mod transport;
// Re-exports (engine::DraftEngine/TargetChain, specdecode::run_spec_decode,
// specpipe::run_spec_decode_pipelined, …) will be added when the worker consumes them.
