use super::IsResponder;
use eyre::Result;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Request {
    /// UUID of the specs request, prevents replay attacks.
    request_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    request_id: String,
    response: String,
}

pub struct SpecResponder;

impl IsResponder for SpecResponder {
    type Request = Request;
    type Response = Response;
}

impl SpecResponder {
    pub fn respond(request: Request) -> Response {
        // TODO: collect specs
    }
}
