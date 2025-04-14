use super::IsResponder;
use dkn_utils::payloads::{SpecRequest, SpecResponse, Specs};

pub struct SpecResponder;

impl IsResponder for SpecResponder {
    type Request = SpecRequest;
    type Response = SpecResponse;
}

impl SpecResponder {
    /// A spec response simply gets the request id of the request and respond back with the same id
    /// along with its spec info.
    #[inline]
    pub fn respond(request: SpecRequest, specs: Specs) -> SpecResponse {
        SpecResponse {
            request_id: request.request_id,
            specs,
        }
    }
}
