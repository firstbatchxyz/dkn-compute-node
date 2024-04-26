use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

/// # Dria Computation Payload
///
/// A computation task is the task of computing a result from a given input. The result is encrypted with the public key of the requester.
/// Plain result is signed by the compute node's private key, and a commitment is computed from the signature and plain result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
/// 
/// ## Fields
/// - `ciphertext`: Computation result encrypted with the public key of the task.
/// - `commitment`: A commitment to `signature || plaintext result`
/// - `signature`: A signature on the digest of plaintext result.
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
