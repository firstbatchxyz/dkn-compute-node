use ecies::encrypt;
use fastbloom_rs::{BloomFilter, Membership};
use libsecp256k1::{sign, Message, PublicKey, RecoveryId, SecretKey, Signature};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};
use tokio::{task::JoinHandle, time};

use crate::{
    config::{constants::WAKU_HEARTBEAT_TOPIC, DriaComputeNodeConfig},
    utils::{
        crypto::{sha256hash, to_address},
        filter::FilterPayload,
        message::{create_content_topic, WakuMessage},
        payload::ComputePayload,
    },
    waku::WakuClient,
};

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub address: [u8; 20],
    pub waku: WakuClient,
}

impl Default for DriaComputeNode {
    fn default() -> Self {
        DriaComputeNode::new(DriaComputeNodeConfig::default())
    }
}

impl DriaComputeNode {
    pub fn new(config: DriaComputeNodeConfig) -> Self {
        let address = to_address(&config.DKN_WALLET_PUBKEY);

        let waku = WakuClient::new(&config.DKN_WAKU_URL);
        DriaComputeNode {
            config,
            address,
            waku,
        }
    }

    /// Returns the wallet address of the node.
    #[inline]
    pub fn address(&self) -> [u8; 20] {
        self.config.DKN_WALLET_ADDRESS
    }

    /// Shorthand to sign a digest with node's secret key.
    #[inline]
    pub fn sign(&self, message: &Message) -> (Signature, RecoveryId) {
        sign(&message, &self.config.DKN_WALLET_PRIVKEY)
    }

    /// Given a hex-string serialized Bloom Filter of a task, checks if this node is selected to do the task.
    ///
    /// This is done by checking if the address of this node is in the filter.
    #[inline]
    pub fn is_tasked(&self, task_filter: String) -> bool {
        BloomFilter::from(FilterPayload::from(task_filter)).contains(&self.address)
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
        let (signature, recid) = sign(&result_msg, &self.config.DKN_WALLET_PRIVKEY);
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
        let commitment: [u8; 32] = sha256hash(preimage);

        Ok(ComputePayload {
            commitment: hex::encode(commitment),
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
        })
    }

    /// Checks for a heartbeat by Dria.
    // pub async fn check_heartbeat(&self) {
    //     let messages = self
    //         .waku
    //         .relay
    //         .get_messages(self.config.DKN_HEARTBEAT_TOPIC.as_str())
    //         .await
    //         .unwrap();

    //     if !messages.is_empty() {
    //         println!("Messages:\n{:?}", messages);
    //         self.handle_heartbeat(&messages[0]).await;
    //     }
    // }

    /// Subscribe to a certain task with its topic.
    pub async fn subscribe_topic(&mut self, content_topic: String) {
        // subscribe to content topic if not subscribed
        if !self.waku.relay.is_subscribed(&content_topic) {
            self.waku
                .relay
                .subscribe(content_topic)
                .await
                .expect("Could not subscribe to topic.");
        }
    }

    /// Checks for a task by Dria. This is done by subscribing to the `task` content topic.
    pub async fn process_topic(&self, topic: String, handler: impl Fn(&Self, Vec<WakuMessage>)) {
        let messages = self.waku.relay.get_messages(topic.as_str()).await.unwrap();
        handler(self, messages);
    }
}
