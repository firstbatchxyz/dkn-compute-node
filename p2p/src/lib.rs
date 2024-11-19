mod transform;

mod behaviour;
use behaviour::{DriaBehaviour, DriaBehaviourEvent};

mod client;
pub use client::DriaP2PClient;

/// Prefix for Kademlia protocol, must start with `/`!
pub const P2P_KADEMLIA_PREFIX: &str = "/dria/kad/";

/// Prefix for Identity protocol string.
pub const P2P_IDENTITY_PREFIX: &str = "dria/";

// re-exports
pub use libp2p;
pub use libp2p_identity;
