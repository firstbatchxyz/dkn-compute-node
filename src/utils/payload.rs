use fastbloom_rs::BloomFilter;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    errors::NodeResult,
    utils::{filter::FilterPayload, get_current_time_nanos},
};

/// A computation task is the task of computing a result from a given input. The result is encrypted with the public key of the requester.
/// Plain result is signed by the compute node's private key, and a commitment is computed from the signature and plain result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponsePayload {
    /// A signature on the digest of plaintext result, prepended with task id.
    pub signature: String,
    /// Computation result encrypted with the public key of the task.
    pub ciphertext: String,
    /// The unique identifier of the task.
    pub task_id: String,
    /// Timestamp of the response.
    pub timestamp: u128,
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
    pub task_id: String,
    /// The deadline of the task in nanoseconds.
    pub(crate) deadline: u128,
    /// The input to the compute function.
    pub(crate) input: T,
    /// The Bloom filter of the task.
    pub(crate) filter: FilterPayload,
    /// The public key of the requester.
    pub(crate) public_key: String,
}

impl<T> TaskRequestPayload<T> {
    #[allow(unused)]
    pub fn new(input: T, filter: BloomFilter, time_ns: u128, public_key: Option<String>) -> Self {
        Self {
            task_id: Uuid::new_v4().into(),
            deadline: get_current_time_nanos() + time_ns,
            input,
            filter: filter.into(),
            public_key: public_key.unwrap_or_default(),
        }
    }
}

/// A parsed `TaskRequestPayload`.
#[derive(Debug, Clone)]
pub struct TaskRequest<T> {
    pub task_id: String,
    pub(crate) input: T,
    pub(crate) public_key: Vec<u8>,
}
