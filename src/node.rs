use ecies::encrypt;
use fastbloom_rs::{BloomFilter, Membership};
use libsecp256k1::{sign, Message, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, to_string};
use tokio::time;

use crate::{
    config::{constants::WAKU_HEARTBEAT_TOPIC, DriaComputeNodeConfig},
    utils::{
        crypto::{sha256hash, to_address},
        filter::FilterPayload,
        message::create_content_topic,
    },
    waku::WakuClient,
};

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct DriaComputeNode {
    secret_key: SecretKey,
    pub public_key: PublicKey,
    pub address: String,
    pub waku: WakuClient,
}

impl Default for DriaComputeNode {
    fn default() -> Self {
        DriaComputeNode::new(DriaComputeNodeConfig::default())
    }
}

impl DriaComputeNode {
    pub fn new(config: DriaComputeNodeConfig) -> Self {
        let secret_key = config.DKN_WALLET_PRIVKEY;
        let public_key = PublicKey::from_secret_key(&secret_key);
        let address = hex::encode(to_address(&public_key));

        let waku = WakuClient::new(&config.DKN_WAKU_URL);
        DriaComputeNode {
            secret_key,
            public_key,
            address,
            waku,
        }
    }

    /// Given a hex-string serialized Bloom Filter of a task, checks if this node is selected to do the task.
    ///
    /// This is done by checking if the address of this node is in the filter.
    #[inline]
    pub fn is_tasked(&self, task_filter: String) -> bool {
        BloomFilter::from(FilterPayload::from(task_filter)).contains(self.address.as_bytes())
    }

    /// Creates the payload of a computation result, as per Dria Whitepaper section 5.1 algorithm 2:
    ///
    /// - Sign result with node `self.secret_key`
    /// - Encrypt (signature, result) with `task_public_key`
    /// - Commit to `(signature, result)` using SHA256.
    pub fn create_payload(
        &self,
        result: impl AsRef<[u8]>,
        task_pubkey: &[u8],
    ) -> Result<ComputePayload, Box<dyn std::error::Error>> {
        // sign result
        let result_digest: [u8; 32] = sha256hash(result.as_ref());
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
        let commitment: [u8; 32] = sha256hash(preimage);

        Ok(ComputePayload {
            commitment: hex::encode(commitment),
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
        })
    }

    /// Checks for a heartbeat by Dria.
    pub async fn check_heartbeat(&mut self) {
        let topic = create_content_topic(WAKU_HEARTBEAT_TOPIC);

        // subscribe to content topic if not subscribed
        if !self.waku.relay.is_subscribed(&topic) {
            self.waku
                .relay
                .subscribe(vec![topic.clone()])
                .await
                .expect("Could not subscribe.");
        }

        loop {
            let messages = self.waku.relay.get_messages(topic.as_str()).await.unwrap();

            if !messages.is_empty() {
                // respond to the latest heartbeat message only
                println!("Messages:\n{:?}", messages);
            }

            time::sleep(time::Duration::from_millis(500)).await;
        }
    }

    /// Checks for a task by Dria. This is done by checking the `task` content topic.
    async fn check_task(&self, topic: String) {
        unimplemented!()
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
pub struct ComputePayload {
    pub signature: String,
    pub ciphertext: String,
    pub commitment: String,
}

impl From<ComputePayload> for String {
    fn from(value: ComputePayload) -> Self {
        to_string(&json!(value)).unwrap()
    }
}
