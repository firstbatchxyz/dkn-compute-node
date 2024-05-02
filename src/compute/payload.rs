use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};

use crate::{errors::NodeResult, utils::filter::FilterPayload};

/// # Dria Task Response
///
/// A computation task is the task of computing a result from a given input. The result is encrypted with the public key of the requester.
/// Plain result is signed by the compute node's private key, and a commitment is computed from the signature and plain result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TaskResponsePayload {
    /// A signature on the digest of plaintext result.
    pub signature: String,
    /// Computation result encrypted with the public key of the task.
    pub ciphertext: String,
    /// A commitment to `signature || result`
    pub commitment: String,
}

impl TaskResponsePayload {
    pub fn to_string(&self) -> NodeResult<String> {
        to_string(&json!(self)).map_err(|e| e.into())
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
/// - `filter`: The Bloom filter of the task.
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
