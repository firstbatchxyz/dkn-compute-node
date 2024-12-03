pub(crate) mod config;
pub(crate) mod handlers;
pub(crate) mod monitor;
pub(crate) mod node;
pub(crate) mod payloads;
pub(crate) mod utils;
pub(crate) mod workers;

/// Crate version of the compute node.
/// This value is attached within the published messages.
pub const DRIA_COMPUTE_NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use utils::refresh_dria_nodes;

pub use config::DriaComputeNodeConfig;
pub use node::DriaComputeNode;

pub use monitor::DriaMonitorNode;
