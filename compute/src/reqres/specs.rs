use crate::utils::Specs;

use super::IsResponder;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SpecRequest {
    /// UUID of the specs request, prevents replay attacks.
    pub request_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct SpecResponse {
    /// UUID of the specs request, prevents replay attacks.
    pub request_id: String,
    /// Node specs, will be flattened during serialization.
    #[serde(flatten)]
    specs: Specs,
}

pub struct SpecResponder;

impl IsResponder for SpecResponder {
    type Request = SpecRequest;
    type Response = SpecResponse;
}

impl SpecResponder {
    pub fn respond(request: SpecRequest, specs: Specs) -> SpecResponse {
        SpecResponse {
            request_id: request.request_id,
            specs,
        }
    }
}
