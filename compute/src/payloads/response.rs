use crate::utils::crypto::{encrypt_bytes, sha256hash, sign_bytes_recoverable};
use eyre::Result;
use libsecp256k1::{PublicKey, SecretKey};
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
    /// Signature of the payload with task id, hexadecimally encoded.
    pub signature: String,
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
        task_id: &str,
        encrypting_public_key: &PublicKey,
        signing_secret_key: &SecretKey,
        model: String,
        stats: TaskStats,
    ) -> Result<Self> {
        // create the message `task_id || payload`
        let mut preimage = Vec::new();
        preimage.extend_from_slice(task_id.as_ref());
        preimage.extend_from_slice(result.as_ref());

        let task_id = task_id.to_string();
        let signature = sign_bytes_recoverable(&sha256hash(preimage), signing_secret_key);
        let ciphertext = encrypt_bytes(result, encrypting_public_key)?;

        Ok(TaskResponsePayload {
            task_id,
            signature,
            ciphertext,
            model,
            stats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecies::decrypt;
    use libsecp256k1::{recover, verify, Message, PublicKey, RecoveryId, Signature};
    use rand::thread_rng;

    #[test]
    fn test_task_response_payload() {
        // this is the result that we are "sending"
        const RESULT: &[u8; 44] = b"hey im an LLM and I came up with this output";
        const MODEL: &str = "gpt-4-turbo";

        // the signer will sign the payload, and it will be verified
        let signer_sk = SecretKey::random(&mut thread_rng());
        let signer_pk = PublicKey::from_secret_key(&signer_sk);

        // the payload will be encrypted with this key
        let task_sk = SecretKey::random(&mut thread_rng());
        let task_pk = PublicKey::from_secret_key(&task_sk);
        let task_id = uuid::Uuid::new_v4().to_string();

        // creates a signed and encrypted payload
        let payload = TaskResponsePayload::new(
            RESULT,
            &task_id,
            &task_pk,
            &signer_sk,
            MODEL.to_string(),
            Default::default(),
        )
        .expect("Should create payload");

        // decrypt result and compare it to plaintext
        let ciphertext_bytes = hex::decode(payload.ciphertext).unwrap();
        let result = decrypt(&task_sk.serialize(), &ciphertext_bytes).expect("Could not decrypt");
        assert_eq!(result, RESULT, "Result mismatch");

        // verify signature
        let signature_bytes = hex::decode(payload.signature).expect("Should decode");
        let signature = Signature::parse_standard_slice(&signature_bytes[..64]).unwrap();
        let recid = RecoveryId::parse(signature_bytes[64]).unwrap();
        let mut preimage = vec![];
        preimage.extend_from_slice(task_id.as_bytes());
        preimage.extend_from_slice(&result);
        let message = Message::parse(&sha256hash(preimage));
        assert!(verify(&message, &signature, &signer_pk), "Could not verify");

        // recover verifying key (public key) from signature
        let recovered_public_key =
            recover(&message, &signature, &recid).expect("Could not recover");
        assert_eq!(signer_pk, recovered_public_key, "Public key mismatch");
    }
}
