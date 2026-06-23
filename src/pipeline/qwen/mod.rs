//! Qwen3-0.6B in Luminal — the bring-up model for the spec-decode pipeline (a small,
//! tokenizer-compatible stand-in while the GLM-5.2 port is built). `model` is the layer-loop
//! forward; `hf` downloads + prepares weights. Both are ported from upstream luminal's qwen
//! example (rev 497b1aa) with the constants adapted to Qwen3-0.6B. The `TargetChain` /
//! `DraftEngine` harness (target.rs) that drives these through the spec-decode driver is next.
#![allow(dead_code)] // model graph + loader are consumed by the TargetChain harness (next step).

pub mod hf;
pub mod model;

pub mod target;

pub mod shard;
