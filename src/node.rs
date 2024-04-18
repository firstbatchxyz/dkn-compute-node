use ecies::encrypt;
use hex::ToHex;
use libsecp256k1::{sign, Message, PublicKey, SecretKey};
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};

use crate::{
    clients::waku::WakuClient,
    utils::{crypto::hash, payload::Payload},
};

/// # Dria Compute Node
///
/// The `secret_key` is constructed from a private key read from the environment.
/// This same key is used by Waku as well.
#[derive(Debug, Clone)]
pub struct DriaComputeNode {
    secret_key: SecretKey,
    pub public_key: PublicKey,
    waku: WakuClient,
    ollama: Ollama,
    model: String,
}

impl Default for DriaComputeNode {
    fn default() -> Self {
        let waku = WakuClient::default();
        let ollama = Ollama::default();
        let secret_key = SecretKey::parse_slice(b"driadriadriadriadriadriadriadria").unwrap();
        DriaComputeNode::new(waku, ollama, secret_key)
    }
}

impl DriaComputeNode {
    pub fn new(waku: WakuClient, ollama: Ollama, secret_key: SecretKey) -> Self {
        let public_key = PublicKey::from_secret_key(&secret_key);

        DriaComputeNode {
            secret_key,
            public_key,
            waku,
            ollama,
            model: "llama2:latest".to_string(), // TODO: make this configurable
        }
    }

    /// Given a bloom-filter of a task, checks if this node is selected to do the task.
    fn check_membership() {
        unimplemented!() // TODO:!
    }

    /// # Dria Computation
    ///
    /// As per Dria Whitepaper section 5.1 algorithm 2:
    ///
    /// - Sign result with node `self.secret_key`
    /// - Encrypt (signature, result) with `task_public_key`
    /// - Commit to `(signature, result)` using SHA256.
    fn create_payload(
        &self,
        result: impl AsRef<[u8]>,
        task_pubkey: &[u8],
    ) -> Result<Payload, Box<dyn std::error::Error>> {
        // sign result
        let result_digest: [u8; 32] = hash(result.as_ref());
        let result_msg = Message::parse_slice(&result_digest)?;
        let (signature, recid) = sign(&result_msg, &self.secret_key);
        let signature: [u8; 64] = signature.serialize();
        let recid: [u8; 1] = [recid.serialize()];

        // encrypt result
        let ciphertext: Vec<u8> =
            encrypt(task_pubkey, result.as_ref()).expect("Could not encrypt.");

        // concat `signature_bytes` and `digest_bytes`
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&signature);
        preimage.extend_from_slice(&recid);
        preimage.extend_from_slice(&result_digest);
        let commitment: [u8; 32] = hash(preimage);

        Ok(Payload {
            commitment: hex::encode(commitment),
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
        })
    }

    /// Computes a result with the given input `data`.
    async fn compute_result(&self, data: &impl AsRef<[u8]>) -> impl AsRef<[u8]> {
        let prompt = String::from_utf8_lossy(data.as_ref()).to_string();

        let gen_req = GenerationRequest::new(self.model.clone(), prompt);
        let res = self.ollama.generate(gen_req).await;
        if let Ok(res) = res {
            return res.response.into();
        }
        vec![/* TODO: unimplemented */]
    }
}

mod tests {
    use super::*;
    use ecies::decrypt;
    use libsecp256k1::{verify, RecoveryId, Signature};

    const RESULT: &[u8; 28] = b"this is some result you know";
    const TASK_PRIV_KEY: &[u8; 32] = b"aaaabbbbccccddddddddccccbbbbaaaa";

    #[test]
    fn test_payload() {
        let node = DriaComputeNode::default();
        let secret_key = SecretKey::parse_slice(TASK_PRIV_KEY).unwrap();
        let public_key = PublicKey::from_secret_key(&secret_key);

        let payload = node
            .create_payload(RESULT, &public_key.serialize())
            .unwrap();

        // decrypt result
        let result = decrypt(
            &secret_key.serialize(),
            hex::decode(payload.ciphertext).unwrap().as_slice(),
        )
        .expect("Could not decrypt.");
        assert_eq!(result, RESULT, "Result mismatch.");

        // verify signature
        let rsv = hex::decode(payload.signature).unwrap();
        let (signature_bytes, recid_bytes) = rsv.split_at(64);
        let signature = Signature::parse_standard_slice(signature_bytes).unwrap();
        let recid = RecoveryId::parse(recid_bytes[0]).unwrap();

        let result_digest = hash(result);
        let message = Message::parse_slice(&result_digest).unwrap();
        assert!(
            verify(&message, &signature, &node.public_key),
            "Could not verify."
        );

        // recover verifying key (public key) from signature
        let recovered_public_key =
            libsecp256k1::recover(&message, &signature, &recid).expect("Could not recover");
        assert_eq!(
            node.public_key, recovered_public_key,
            "Public key mismatch."
        );

        // verify commitments
        let mut preimage = Vec::new();
        preimage.extend_from_slice(signature_bytes);
        preimage.extend_from_slice(recid_bytes);
        preimage.extend_from_slice(&result_digest);
        assert_eq!(
            hash(preimage),
            hex::decode(payload.commitment).unwrap().as_slice(),
            "Commitment mismatch."
        );
    }
}
