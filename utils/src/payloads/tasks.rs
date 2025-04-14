use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};

/// Topic used within [`crate::DriaMessage`] for task result messages.
pub const TASK_RESULT_TOPIC: &str = "results";

/// A computation task is the task of computing a result from a given input. The result is encrypted with the public key of the requester.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponsePayload {
    /// The unique identifier of the task.
    ///
    /// It is formed of two parts: the task id and the RPC auth id, splitted by `--`.
    pub task_id: String,
    /// Result encrypted with the public key of the task, Hexadecimally encoded.
    pub ciphertext: String,
    /// Name of the model used for this task.
    pub model: String,
    /// Stats about the task execution.
    pub stats: TaskStats,
}

/// A task error response.
/// Returning this as the payload helps to debug the errors received at client side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskErrorPayload {
    /// The unique identifier of the task.
    pub task_id: String,
    /// The stringified error object
    pub error: String,
    /// Name of the model that caused the error.
    pub model: String,
    /// Task statistics.
    pub stats: TaskStats,
}

impl TaskResponsePayload {
    /// Creates the payload of a computation with its (encrypted) result.
    pub fn new(
        result: impl AsRef<[u8]>,
        task_id: impl ToString,
        task_pk: &PublicKey,
        model: String,
        stats: TaskStats,
    ) -> Result<Self, libsecp256k1::Error> {
        let ciphertext = ecies::encrypt(&task_pk.serialize(), result.as_ref())?;

        Ok(TaskResponsePayload {
            task_id: task_id.to_string(),
            ciphertext: hex::encode(ciphertext),
            model,
            stats,
        })
    }
}

/// A generic task request, given by Dria.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequestPayload<T> {
    /// The unique identifier of the task.
    pub task_id: String,
    /// The deadline of the task.
    pub deadline: chrono::DateTime<chrono::Utc>,
    /// The input to the compute function.
    pub input: T,
    /// The public key of the requester, in hexadecimals.
    pub public_key: String,
}

/// Task stats for diagnostics.
///
/// Returning this as the payload helps to debug the errors received at client side, and latencies.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStats {
    /// Timestamp at which the task was received from network & parsed.
    pub received_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp at which the task was published back to network.
    pub published_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp at which the task execution had started.
    pub execution_started_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp at which the task execution had finished.
    pub execution_ended_at: chrono::DateTime<chrono::Utc>,
}

impl TaskStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the current timestamp within `received_at`.
    pub fn record_received_at(mut self) -> Self {
        self.received_at = chrono::Utc::now();
        self
    }

    /// Records the current timestamp within `published_at`.
    pub fn record_published_at(mut self) -> Self {
        self.published_at = chrono::Utc::now();
        self
    }

    /// Records the execution start time within `execution_started_at`.
    pub fn record_execution_started_at(mut self) -> Self {
        self.execution_started_at = chrono::Utc::now();
        self
    }

    /// Records the execution end time within `execution_ended_time`.
    pub fn record_execution_ended_at(mut self) -> Self {
        self.execution_ended_at = chrono::Utc::now();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecies::{decrypt, PublicKey, SecretKey};

    #[test]
    fn test_task_response_payload() {
        const DUMMY_SECRET_KEY: &[u8; 32] = b"driadriadriadriadriadriadriadria";

        // this is the result that we are "sending"
        const RESULT: &[u8; 44] = b"hey im an LLM and I came up with this output";
        const MODEL: &str = "gpt-4-turbo";

        // the payload will be encrypted with this key
        let task_sk = SecretKey::parse(&DUMMY_SECRET_KEY).unwrap();
        let task_pk = PublicKey::from_secret_key(&task_sk);
        let task_id = uuid::Uuid::new_v4().to_string();

        // creates a signed and encrypted payload
        let payload = TaskResponsePayload::new(
            RESULT,
            &task_id,
            &task_pk,
            MODEL.to_string(),
            Default::default(),
        )
        .expect("to create payload");

        // decrypt result and compare it to plaintext
        let ciphertext_bytes = hex::decode(payload.ciphertext).unwrap();
        let result = decrypt(&task_sk.serialize(), &ciphertext_bytes).expect("to decrypt");
        assert_eq!(result, RESULT, "Result mismatch");
    }
}
