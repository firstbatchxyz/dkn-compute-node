use ecies::encrypt;
use fastbloom_rs::{BloomFilter, Membership};
use libsecp256k1::{sign, verify, Message, RecoveryId, Signature};

use crate::{
    config::DriaComputeNodeConfig,
    utils::{
        crypto::sha256hash, filter::FilterPayload, message::WakuMessage, payload::ComputePayload,
    },
    waku::WakuClient,
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
        let waku = WakuClient::new(&config.DKN_WAKU_URL);
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
        sign(&message, &self.config.DKN_WALLET_SECRET_KEY)
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
    pub fn is_tasked(&self, filter: FilterPayload) -> bool {
        BloomFilter::from(filter).contains(&self.address())
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
    ) -> Result<ComputePayload, Box<dyn std::error::Error>> {
        // sign result
        let result_digest: [u8; 32] = sha256hash(result.as_ref());
        let result_msg = Message::parse(&result_digest);
        let (signature, recid) = sign(&result_msg, &self.config.DKN_WALLET_SECRET_KEY);
        let signature: [u8; 64] = signature.serialize();
        let recid: [u8; 1] = [recid.serialize()];

        // encrypt result
        let ciphertext = encrypt(task_pubkey, result.as_ref()).expect("Could not encrypt.");

        // concat `signature_bytes` and `digest_bytes`
        let mut preimage = Vec::new();
        preimage.extend_from_slice(&signature);
        preimage.extend_from_slice(&recid);
        preimage.extend_from_slice(&result_digest);
        let commitment: [u8; 32] = sha256hash(preimage);

        Ok(ComputePayload {
            commitment: hex::encode(commitment),
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
        })
    }

    /// Subscribe to a certain task with its topic.
    pub async fn subscribe_topic(
        &mut self,
        content_topic: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.waku.relay.is_subscribed(&content_topic) {
            self.waku.relay.subscribe(content_topic).await
        } else {
            Ok(())
        }
    }

    /// Processes messages in a topic with a handler.
    ///
    pub async fn process_topic(
        &self,
        topic: String,
    ) -> Result<Vec<WakuMessage>, Box<dyn std::error::Error>> {
        let mut messages: Vec<WakuMessage> = self.waku.relay.get_messages(topic.as_str()).await?;

        // only keep messages that are authentic to Dria
        messages.retain(|message| {
            let (signature, rest) = message.payload.split_at(65);
            let (signature, _) = signature.split_at(1);
            let signature = Signature::parse_standard_slice(signature.as_bytes());
            match signature {
                // signature could be parsed, return its verification result
                Ok(signature) => verify(
                    &Message::parse(&sha256hash(rest.as_bytes())),
                    &signature,
                    &self.config.DKN_ADMIN_PUBLIC_KEY,
                ),
                // signature could not be parsed
                Err(_) => false,
            }
        });

        // map each message that is `signature || payload` into just the `payload`
        let messages = messages
            .into_iter()
            .map(|mut message| {
                message.payload = message.payload[66..].to_string();
                return message;
            })
            .collect();

        Ok(messages)
    }
}
