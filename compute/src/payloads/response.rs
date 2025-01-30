use eyre::Result;
use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};

use super::TaskStats;

/// A computation task is the task of computing a result from a given input. The result is encrypted with the public key of the requester.
/// Plain result is signed by the compute node's private key, and a commitment is computed from the signature and plain result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponsePayload {
    /// The unique identifier of the task.
    pub task_id: String,
    /// Result encrypted with the public key of the task, Hexadecimally encoded.
    pub ciphertext: String,
    /// Name of the model used for this task.
    pub model: String,
    /// Stats about the task execution.
    pub stats: TaskStats,
}

impl TaskResponsePayload {
    /// Creates the payload of a computation result.
    ///
    /// - Sign `task_id || payload` with node `self.secret_key`
    /// - Encrypt `result` with `task_public_key`
    pub fn new(
        result: impl AsRef<[u8]>,
        task_id: impl ToString,
        task_pk: &PublicKey,
        model: String,
        stats: TaskStats,
    ) -> Result<Self> {
        let ciphertext = ecies::encrypt(&task_pk.serialize(), result.as_ref())?;

        Ok(TaskResponsePayload {
            task_id: task_id.to_string(),
            ciphertext: hex::encode(ciphertext),
            model,
            stats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecies::{decrypt, PublicKey, SecretKey};
    use rand::thread_rng;

    #[test]
    fn test_task_response_payload() {
        // this is the result that we are "sending"
        const RESULT: &[u8; 44] = b"hey im an LLM and I came up with this output";
        const MODEL: &str = "gpt-4-turbo";

        // the payload will be encrypted with this key
        let task_sk = SecretKey::random(&mut thread_rng());
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
