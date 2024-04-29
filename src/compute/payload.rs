use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

use crate::utils::filter::FilterPayload;

/// # Dria Task Response
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TaskResponsePayload {
    pub signature: String,
    pub ciphertext: String,
    pub commitment: String,
}

impl From<TaskResponsePayload> for String {
    fn from(value: TaskResponsePayload) -> Self {
        to_string(&json!(value)).unwrap()
    }
}

/// # Dria Task Request
///
/// A task request TODO: ...
#[derive(Serialize, Deserialize, Debug, Clone)]
struct TaskRequestPayload<T> {
    task_id: String,
    deadline: u128,
    input: T,
    filter: FilterPayload,
    public_key: String,
}
