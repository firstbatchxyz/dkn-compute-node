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
/// A generic task request, given by Dria.
///
/// ## Fields
///
/// - `task_id`: The unique identifier of the task.
/// - `deadline`: The deadline of the task in nanoseconds.
/// - `input`: The input to the compute function.
/// - `filter`: The filter of the task.
/// - `public_key`: The public key of the requester.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestPayload<T> {
    pub(crate) task_id: String,
    pub(crate) deadline: u128,
    pub(crate) input: T,
    pub(crate) filter: FilterPayload,
    pub(crate) public_key: String,
}
