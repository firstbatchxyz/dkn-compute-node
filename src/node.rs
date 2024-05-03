use ecies::encrypt;
use fastbloom_rs::{BloomFilter, Membership};
use libsecp256k1::{sign, Message, RecoveryId, Signature};

use crate::{
    compute::payload::TaskResponsePayload,
    config::DriaComputeNodeConfig,
    errors::NodeResult,
    utils::{crypto::sha256hash, filter::FilterPayload},
    waku::{message::WakuMessage, WakuClient},
};

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub waku: WakuClient,
}

impl Default for DriaComputeNode {
    fn default() -> Self {
        DriaComputeNode::new(DriaComputeNodeConfig::default())
    }
}

impl DriaComputeNode {
    pub fn new(config: DriaComputeNodeConfig) -> Self {
        let waku = WakuClient::new(None);
        DriaComputeNode { config, waku }
    }

    /// Returns the wallet address of the node.
    #[inline]
    pub fn address(&self) -> [u8; 20] {
        self.config.DKN_WALLET_ADDRESS
    }

    /// Shorthand to sign a digest with node's secret key and return signature & recovery id.
    #[inline]
    pub fn sign(&self, message: &Message) -> (Signature, RecoveryId) {
        sign(message, &self.config.DKN_WALLET_SECRET_KEY)
    }

    /// Shorthand to sign a digest (bytes) with node's secret key and return signature & recovery id
    /// serialized to 65 byte hex-string.
    #[inline]
    pub fn sign_bytes(&self, message: &[u8; 32]) -> String {
        let message = Message::parse(message);
        let (signature, recid) = sign(&message, &self.config.DKN_WALLET_SECRET_KEY);

        format!(
            "{}{}",
            hex::encode(signature.serialize()),
            hex::encode([recid.serialize()])
        )
    }

    /// Given a hex-string serialized Bloom Filter of a task, checks if this node is selected to do the task.
    ///
    /// This is done by checking if the address of this node is in the filter.
    #[inline]
    pub fn is_tasked(&self, filter: &FilterPayload) -> NodeResult<bool> {
        let filter = BloomFilter::try_from(filter)?;

        Ok(filter.contains(&self.address()))
    }

    /// Creates the payload of a computation result, as per Dria Whitepaper section 5.1 algorithm 2:
    ///
    /// - Sign result with node `self.secret_key`
    /// - Encrypt `(signature || result)` with `task_public_key`
    /// - Commit to `(signature || result)` using SHA256.
    pub fn create_payload(
        &self,
        result: impl AsRef<[u8]>,
        task_pubkey: &[u8],
    ) -> NodeResult<TaskResponsePayload> {
        // sign result
        let result_digest: [u8; 32] = sha256hash(result.as_ref());
        let result_msg = Message::parse(&result_digest);
        let (signature, recid) = sign(&result_msg, &self.config.DKN_WALLET_SECRET_KEY);
        let signature: [u8; 64] = signature.serialize();
        let recid: [u8; 1] = [recid.serialize()];

        // encrypt result
        let ciphertext = encrypt(task_pubkey, result.as_ref())?;

        // concatenate `signature_bytes` and `digest_bytes`
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&signature);
        preimage.extend_from_slice(&recid);
        preimage.extend_from_slice(&result_digest);
        let commitment: [u8; 32] = sha256hash(preimage);

        Ok(TaskResponsePayload {
            commitment: hex::encode(commitment),
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
        })
    }

    /// Subscribe to a certain task with its topic.
    pub async fn subscribe_topic(&self, topic: &str) -> NodeResult<()> {
        let content_topic = WakuMessage::create_content_topic(topic);
        self.waku.relay.subscribe(&content_topic).await?;
        log::info!("Subscribed to {}", topic);
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic.
    pub async fn unsubscribe_topic(&self, topic: &str) -> NodeResult<()> {
        let content_topic = WakuMessage::create_content_topic(topic);
        self.waku.relay.unsubscribe(&content_topic).await?;
        log::info!("Unsubscribed from {}", topic);
        Ok(())
    }

    /// Send a message via Waku Relay.
    pub async fn send_message(&self, message: WakuMessage) -> NodeResult<()> {
        self.waku.relay.send_message(message).await
    }

    /// Send a message via Waku Relay on a topic, where
    /// the topic is subscribed, the message is sent, and
    /// the topic is unsubscribed right afterwards.
    pub async fn send_once_message(&self, message: WakuMessage) -> NodeResult<()> {
        let content_topic = message.content_topic.clone();
        self.waku.relay.subscribe(&content_topic).await?;
        self.waku.relay.send_message(message).await?;
        self.waku.relay.unsubscribe(&content_topic).await?;
        Ok(())
    }

    /// Process messages on a certain topic, and if they are expected to be signed by the admin
    /// key of Dria, only keeps the ones that are authentic.
    pub async fn process_topic(&self, topic: &str, signed: bool) -> NodeResult<Vec<WakuMessage>> {
        let content_topic = WakuMessage::create_content_topic(topic);
        let mut messages: Vec<WakuMessage> = self.waku.relay.get_messages(&content_topic).await?;
        if !messages.is_empty() {
            log::debug!("Received messages on topic {}:", topic);
            for message in &messages {
                log::debug!("{}", message);
            }
        }

        // if signed, only keep messages that are authentic to Dria
        if signed {
            messages.retain(|message| {
                message
                    .is_signed(&self.config.DKN_ADMIN_PUBLIC_KEY)
                    .unwrap_or_else(|e| {
                        log::warn!("Could not verify message signature: {}", e);
                        false
                    })
            });
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecies::decrypt;
    use libsecp256k1::{verify, PublicKey, SecretKey};

    /// This test demonstrates the creation and parsing of a payload.
    ///
    /// In DKN, the payload is created by Compute Node but parsed by the Admin Node.
    /// At the end, there is also the verification step for the commitments.
    #[test]
    fn test_payload_generation_verification() {
        const ADMIN_PRIV_KEY: &[u8; 32] = b"aaaabbbbccccddddddddccccbbbbaaaa";
        const RESULT: &[u8; 28] = b"this is some result you know";

        let node = DriaComputeNode::default();
        let secret_key = SecretKey::parse(ADMIN_PRIV_KEY).expect("Should parse secret key");
        let public_key = PublicKey::from_secret_key(&secret_key);

        // create payload
        let payload = node
            .create_payload(RESULT, &public_key.serialize())
            .expect("Should create payload");

        // (here we assume the payload is sent to Waku network, and picked up again)

        // decrypt result
        let result = decrypt(
            &secret_key.serialize(),
            hex::decode(payload.ciphertext)
                .expect("Should decode")
                .as_slice(),
        )
        .expect("Could not decrypt");
        assert_eq!(result, RESULT, "Result mismatch");

        // verify signature
        let rsv = hex::decode(payload.signature).expect("Should decode");
        let mut signature_bytes = [0u8; 64];
        signature_bytes.copy_from_slice(&rsv[0..64]);
        let recid_bytes: [u8; 1] = [rsv[64]];
        let signature =
            Signature::parse_standard(&signature_bytes).expect("Should parse signature");
        let recid = RecoveryId::parse(recid_bytes[0]).expect("Should parse recovery id");

        let result_digest = sha256hash(result);
        let message = Message::parse(&result_digest);
        assert!(
            verify(&message, &signature, &node.config.DKN_WALLET_PUBLIC_KEY),
            "Could not verify"
        );

        // recover verifying key (public key) from signature
        let recovered_public_key =
            libsecp256k1::recover(&message, &signature, &recid).expect("Could not recover");
        assert_eq!(
            node.config.DKN_WALLET_PUBLIC_KEY, recovered_public_key,
            "Public key mismatch"
        );

        // verify commitments (algorithm 4 in whitepaper)
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&signature_bytes);
        preimage.extend_from_slice(&recid_bytes);
        preimage.extend_from_slice(&result_digest);
        assert_eq!(
            sha256hash(preimage),
            hex::decode(payload.commitment)
                .expect("Should decode")
                .as_slice(),
            "Commitment mismatch"
        );
    }
}
