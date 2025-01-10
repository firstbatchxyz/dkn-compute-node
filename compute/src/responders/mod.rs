use eyre::Context;
use serde::{de::DeserializeOwned, Serialize};

mod specs;
pub use specs::SpecResponder;

mod workflow;
pub use workflow::WorkflowResponder;

/// A responder should implement a request & response type, both serializable.
///
/// The `try_parse_request` is automatically implemented using `serde-json` for a byte slice.
pub trait IsResponder {
    type Request: DeserializeOwned;
    type Response: Serialize + DeserializeOwned;

    fn try_parse_request(data: &[u8]) -> eyre::Result<Self::Request> {
        serde_json::from_slice(data).wrap_err("could not parse request")
    }
}
