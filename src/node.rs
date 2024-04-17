use ecies::encrypt;
use libsecp256k1::{sign, Message, PublicKey, SecretKey, Signature};
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use serde::{Deserialize, Serialize};

use crate::{
    clients::waku::WakuClient,
    utils::crypto::{hash, SignatureVRS},
};

/// # Dria Compute Node
///
/// The `secret_key` is constructed from a private key read from the environment.
/// This same key is used by Waku as well.
#[derive(Debug, Clone)]
pub struct DriaComputeNode {
    secret_key: SecretKey,
    public_key: PublicKey,
    waku: WakuClient,
    ollama: Ollama,
    model: String,
}

impl Default for DriaComputeNode {
    fn default() -> Self {
        let waku = WakuClient::default();
        let ollama = Ollama::default();
        DriaComputeNode::new(waku, ollama)
    }
}

impl DriaComputeNode {
    pub fn new(waku: WakuClient, ollama: Ollama) -> Self {
        let secret_key = SecretKey::parse_slice(&[1] /* TODO: read from env */).unwrap();
        let public_key = PublicKey::from_secret_key(&secret_key);

        DriaComputeNode {
            secret_key,
            public_key,
            waku,
            ollama,
            model: "llama2:latest".to_string(), // TODO: make this configurable
        }
    }

    /// # Dria Computation
    ///
    /// As per Dria Whitepaper section 5.1 algorithm 2:
    ///
    /// 1. Compute result
    /// 2. Sign result with node `self.secret_key`
    /// 3. Encrypt (signature, result) with `task_public_key`
    /// 4. Commit to `(signature, result)` using SHA256.
    async fn create_payload(
        &self,
        data: impl AsRef<[u8]>,
        task_pubkey: &[u8],
    ) -> Result<ComputationPayload, Box<dyn std::error::Error>> {
        let result = self.compute_result(&data).await;

        // sign result
        let digest_bytes = hash(data);
        let digest_msg = Message::parse_slice(&digest_bytes)?;
        let (signature, recid) = sign(&digest_msg, &self.secret_key);

        // encrypt result
        let ciphertext = encrypt(task_pubkey, result.as_slice()).expect("Could not encrypt.");

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
    async fn compute_result(&self, data: &impl AsRef<[u8]>) -> Vec<u8> {
        let prompt = String::from_utf8_lossy(data.as_ref()).to_string();

        let gen_req = GenerationRequest::new(self.model.clone(), prompt);
        let res = self.ollama.generate(gen_req).await;
        if let Ok(res) = res {
            return res.response.into();
        }
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
#[derive(Serialize, Deserialize, Debug)]
pub struct ComputationPayload {
    signature: SignatureVRS,
    ciphertext: Vec<u8>,
    commitment: [u8; 32],
    // TODO: recovery Id?
}
