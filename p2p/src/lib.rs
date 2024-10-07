mod behaviour;
pub use behaviour::{DriaBehaviour, DriaBehaviourEvent};

mod client;
pub use client::P2PClient;

mod versioning;
pub use versioning::*;

mod data_transform;

// re-exports

pub use libp2p;
pub use libp2p_identity;
