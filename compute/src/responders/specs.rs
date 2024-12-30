use crate::utils::Specs;

use super::IsResponder;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Request {
    /// UUID of the specs request, prevents replay attacks.
    request_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    request_id: String,
    #[serde(flatten)]
    specs: Specs,
}

pub struct SpecResponder;

impl IsResponder for SpecResponder {
    type Request = Request;
    type Response = Response;
}

impl SpecResponder {
    pub fn respond(request: Request, specs: Specs) -> Response {
        Response {
            request_id: request.request_id,
            specs,
        }
    }
}
