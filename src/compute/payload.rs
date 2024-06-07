use serde::{Deserialize, Serialize};

use crate::{errors::NodeResult, utils::filter::FilterPayload};

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
    /// A commitment to `signature || result`.
    pub commitment: String,
}

impl TaskResponsePayload {
    pub fn to_string(&self) -> NodeResult<String> {
        serde_json::to_string(&serde_json::json!(self)).map_err(|e| e.into())
    }
}

/// A generic task request, given by Dria.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestPayload<T> {
    /// The unique identifier of the task.
    pub(crate) task_id: String,
    /// The deadline of the task in nanoseconds.
    pub(crate) deadline: u128,
    /// The input to the compute function.
    pub(crate) input: T,
    /// The Bloom filter of the task.
    pub(crate) filter: FilterPayload,
    /// The public key of the requester.
    pub(crate) public_key: String,
}

/// A parsed `TaskRequestPayload`.
pub struct TaskRequest<T> {
    pub(crate) task_id: String,
    pub(crate) input: T,
    pub(crate) public_key: Vec<u8>,
}
