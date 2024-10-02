use crate::utils::{filter::FilterPayload, get_current_time_nanos};
use fastbloom_rs::BloomFilter;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A generic task request, given by Dria.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// The public key of the requester, in hexadecimals.
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
