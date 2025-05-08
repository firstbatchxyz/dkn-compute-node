pub mod config;
pub mod node;
pub mod reqres;
pub mod utils;
pub mod workers;

/// Crate version of the compute node.
/// This value is attached within the published messages.
pub const DRIA_COMPUTE_NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use config::DriaComputeNodeConfig;
pub use node::DriaComputeNode;
