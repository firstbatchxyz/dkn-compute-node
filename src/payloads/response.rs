use crate::utils::crypto::sha256hash;
use eyre::Result;
use libsecp256k1::SecretKey;
use serde::{Deserialize, Serialize};

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
}

impl TaskResponsePayload {
    /// Creates the payload of a computation result.
    ///
    /// - Sign `task_id || payload` with node `self.secret_key`
    /// - Encrypt `result` with `task_public_key`
    pub fn new(
        result: impl AsRef<[u8]>,
        task_id: &str,
        encrypting_public_key: &[u8],
        signing_secret_key: &SecretKey,
    ) -> Result<Self> {
        // create the message `task_id || payload`
        let mut preimage = Vec::new();
        preimage.extend_from_slice(task_id.as_ref());
        preimage.extend_from_slice(result.as_ref());

        // sign the message
        // TODO: use `sign_recoverable` here instead?
        let digest = libsecp256k1::Message::parse(&sha256hash(preimage));
        let (signature, recid) = libsecp256k1::sign(&digest, signing_secret_key);
        let signature: [u8; 64] = signature.serialize();
        let recid: [u8; 1] = [recid.serialize()];

        // encrypt payload itself
        let ciphertext = ecies::encrypt(encrypting_public_key, result.as_ref())?;

        Ok(TaskResponsePayload {
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
            task_id: task_id.to_string(),
        })
    }
}
