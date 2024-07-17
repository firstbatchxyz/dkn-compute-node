#![doc = include_str!("../README.md")]

pub(crate) mod config;
pub(crate) mod errors;
pub(crate) mod handlers;
pub(crate) mod node;
pub(crate) mod p2p;
pub(crate) mod utils;

/// Crate version of the compute node.
/// This value is attached within the published messages.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use config::DriaComputeNodeConfig;
pub use node::DriaComputeNode;
