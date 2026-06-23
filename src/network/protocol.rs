// Full protocol surface re-export. Some types (ChatMessage, ResponseFormat, ValidationRequest,
// TaskStats, RejectReason, ModelType) are consumed once the Luminal pipeline worker is wired.
#[allow(unused_imports)]
pub use dkn_protocol::{
    read_framed, write_framed, Capacity, ChatMessage, ModelType, NodeMessage, RejectReason,
    ResponseFormat, RouterMessage, TaskStats, ValidationRequest,
};
