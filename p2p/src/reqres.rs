use serde::{Deserialize, Serialize};

/// Request-Response protocol, request type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReqresRequest {
    pub request_id: String,
}

/// Request-Response protocol, response type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReqresResponse {
    pub spec: String,
    pub location: String,
}
