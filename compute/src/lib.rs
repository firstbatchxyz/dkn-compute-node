pub mod config;
pub mod handlers;
pub mod launch;
pub mod node;
pub mod payloads;
pub mod utils;
pub mod workers;

/// Crate version of the compute node.
/// This value is attached within the published messages.
pub const DRIA_COMPUTE_NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use config::DriaComputeNodeConfig;
pub use launch::launch;
pub use node::DriaComputeNode;
pub use utils::refresh_dria_nodes;
