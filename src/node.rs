use ecies::encrypt;
use libsecp256k1::{sign, Message, PublicKey, SecretKey, Signature};
use ollama_rs::Ollama;

use crate::{
    clients::waku::WakuClient,
    utils::crypto::{hash, SignatureVRS},
};

/// # Dria Compute Node
///
/// The `secret_key` is constructed from a private key read from the environment.
/// This same key is used by Waku as well.
pub struct DriaComputeNode {
    secret_key: SecretKey,
    public_key: PublicKey,
    waku: WakuClient,
    ollama: Ollama,
}

impl DriaComputeNode {
    /// Creates a new instance of WakuClient.
    pub fn new(waku_url: Option<&str>, ollama_host: Option<&str>) -> Self {
        let secret_key = SecretKey::parse_slice(&[1] /* TODO: read from env */).unwrap();
        let public_key = PublicKey::from_secret_key(&secret_key);

        let waku = WakuClient::new(waku_url);
        let ollama = Ollama::default();

        DriaComputeNode {
            secret_key,
            public_key,
            waku,
            ollama,
        }
    }

    /// Dria Computation (Dria Whitepaper sec. 5.1 alg: 2)
    ///
    /// 1. Compute result
    /// 2. Sign result with node `self.secret_key`
    /// 3. Encrypt (signature, result) with `task_public_key`
    /// 4. Commit to `(signature, result)` using SHA256.
    fn create_payload(
        &self,
        data: impl AsRef<[u8]>,
        task_public_key_serialized: &[u8],
    ) -> Result<ComputationPayload, Box<dyn std::error::Error>> {
        let result = self.compute_result(&data);

        // sign result
        let digest_bytes = hash(data);
        let digest_msg = Message::parse_slice(&digest_bytes)?;
        let (signature, recid) = sign(&digest_msg, &self.secret_key);

        // encrypt result
        let ciphertext =
            encrypt(task_public_key_serialized, result.as_slice()).expect("Could not encrypt.");

        // concat `signature_bytes` and `digest_bytes`
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&signature.serialize());
        preimage.extend_from_slice(&digest_bytes);
        let commitment = hash(preimage);

        Ok(ComputationPayload {
            commitment,
            ciphertext,
            signature: SignatureVRS::from((signature, recid)),
        })
    }

    /// Computes a result with the given input `data`.
    fn compute_result(&self, data: &impl AsRef<[u8]>) -> Vec<u8> {
        vec![/* TODO: unimplemented */]
    }
}

/// A Dria Computation payload is a triple:
///
/// 1. Ciphertext: Computation result encrypted with the public key of the task.
/// 2. Commitment: A commitment to `signature || plaintext result`
/// 3. Signature: A signature on the digest of plaintext result.
///
/// To check the commitment, one must decrypt the ciphertext and parse plaintext from it,
/// and compute the digest using SHA256. That digest will then be used for the signature check.
struct ComputationPayload {
    signature: SignatureVRS,
    ciphertext: Vec<u8>,
    commitment: [u8; 32],
    // TODO: recovery Id?
}
