mod specs;
use eyre::Context;
use serde::{de::DeserializeOwned, Serialize};
pub use specs::SpecResponder;

pub trait IsResponder {
    type Request: Serialize + DeserializeOwned;
    type Response: Serialize + DeserializeOwned;

    fn try_parse_request<'a>(data: &[u8]) -> eyre::Result<Self::Request> {
        serde_json::from_slice(data).wrap_err("could not parse request")
    }
}
