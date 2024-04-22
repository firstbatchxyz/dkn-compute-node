use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

/// A Dria Computation payload is a triple:
///
/// 1. Ciphertext: Computation result encrypted with the public key of the task.
/// 2. Commitment: A commitment to `signature || plaintext result`
/// 3. Signature: A signature on the digest of plaintext result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
#[derive(Serialize, Deserialize, Debug)]
pub struct ComputePayload {
    pub signature: String,
    pub ciphertext: String,
    pub commitment: String,
}

impl From<ComputePayload> for String {
    fn from(value: ComputePayload) -> Self {
        to_string(&json!(value)).unwrap()
    }
}
