use ecies::encrypt;
use fastbloom_rs::{BloomFilter, Membership};
use libsecp256k1::{sign, Message, RecoveryId, Signature};
use parking_lot::RwLock;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::{
    config::DriaComputeNodeConfig,
    errors::NodeResult,
    utils::payload::{TaskRequest, TaskRequestPayload, TaskResponsePayload},
    utils::{crypto::sha256hash, filter::FilterPayload, get_current_time_nanos},
    waku::{message::WakuMessage, WakuClient},
};

#[allow(unused)]
#[derive(Debug)]
pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub waku: WakuClient,
    pub cancellation: CancellationToken,
    pub busy_lock: RwLock<bool>,
}

impl Default for DriaComputeNode {
    fn default() -> Self {
        DriaComputeNode::new(DriaComputeNodeConfig::new(), CancellationToken::default())
    }
}

impl DriaComputeNode {
    pub fn new(config: DriaComputeNodeConfig, cancellation: CancellationToken) -> Self {
        let waku = WakuClient::new(None);
        let busy_lock = RwLock::new(false);
        DriaComputeNode {
            config,
            waku,
            cancellation,
            busy_lock,
        }
    }

    /// Shorthand to sign a digest with node's secret key and return signature & recovery id.
    #[inline]
    pub fn sign(&self, message: &Message) -> (Signature, RecoveryId) {
        sign(message, &self.config.secret_key)
    }

    /// Returns the state of the node, whether it is busy or not.
    #[inline]
    pub fn is_busy(&self) -> bool {
        *self.busy_lock.read()
    }

    /// Set the state of the node, whether it is busy or not.
    #[inline]
    pub fn set_busy(&self, busy: bool) {
        *self.busy_lock.write() = busy;
    }

    /// Given a hex-string serialized Bloom Filter of a task, checks if this node is selected to do the task.
    ///
    /// This is done by checking if the address of this node is in the filter.
    #[inline]
    pub fn is_tasked(&self, filter: &FilterPayload) -> NodeResult<bool> {
        let filter = BloomFilter::try_from(filter)?;

        Ok(filter.contains(&self.config.address))
    }

    /// Creates the payload of a computation result, as per Dria Whitepaper section 5.1 algorithm 2:
    ///
    /// - Sign `task_id || result` with node `self.secret_key`
    /// - Encrypt `result` with `task_public_key`
    pub fn create_payload(
        &self,
        result: impl AsRef<[u8]>,
        task_id: impl AsRef<[u8]>,
        task_pubkey: &[u8],
    ) -> NodeResult<TaskResponsePayload> {
        // sign result
        let mut preimage = Vec::new();
        preimage.extend_from_slice(task_id.as_ref());
        preimage.extend_from_slice(result.as_ref());
        let digest = Message::parse(&sha256hash(preimage));
        let (signature, recid) = sign(&digest, &self.config.secret_key);
        let signature: [u8; 64] = signature.serialize();
        let recid: [u8; 1] = [recid.serialize()];

        // encrypt result
        let ciphertext = encrypt(task_pubkey, result.as_ref())?;

        Ok(TaskResponsePayload {
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
        })
    }

    /// Subscribe to a certain task with its topic.
    pub async fn subscribe_topic(&self, topic: &str) {
        let content_topic = WakuMessage::create_content_topic(topic);

        const MAX_RETRIES: usize = 30;
        let mut retry_count = 0; // retry count for edge case
        while let Err(e) = self.waku.relay.subscribe(&content_topic).await {
            if retry_count < MAX_RETRIES {
                log::error!(
                    "Error subscribing to {}: {}\nRetrying in 5 seconds ({}/{}).",
                    topic,
                    e,
                    retry_count,
                    MAX_RETRIES
                );
                tokio::select! {
                    _ = self.cancellation.cancelled() => return,
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                        retry_count += 1; // Increment the retry counter
                    }
                }
            } else {
                log::error!("Error subscribing to {}: {}\nAborting.", topic, e);
                self.cancellation.cancel();
            }
        }

        log::info!("Subscribed to {}", topic);
    }

    /// Unsubscribe from a certain task with its topic.
    pub async fn unsubscribe_topic(&self, topic: &str) -> NodeResult<()> {
        let content_topic = WakuMessage::create_content_topic(topic);
        self.waku.relay.unsubscribe(&content_topic).await?;
        log::info!("Unsubscribed from {}", topic);
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic, ignoring the error.
    pub async fn unsubscribe_topic_ignored(&self, topic: &str) {
        if let Err(e) = self.unsubscribe_topic(topic).await {
            log::error!(
                "Error unsubscribing from {}: {}\nContinuing anyway.",
                topic,
                e
            );
        }
    }

    /// Send a message via Waku Relay, assuming the content is subscribed to already.
    pub async fn send_message(&self, message: WakuMessage) -> NodeResult<()> {
        self.waku.relay.send_message(message).await
    }

    /// Send a message via Waku Relay on a topic, where
    /// the topic is subscribed, the message is sent, and
    /// the topic is unsubscribed right afterwards.
    pub async fn send_message_once(&self, message: WakuMessage) -> NodeResult<()> {
        let content_topic = message.content_topic.clone();
        self.waku.relay.subscribe(&content_topic).await?;
        self.waku.relay.send_message(message).await?;
        self.waku.relay.unsubscribe(&content_topic).await?;
        Ok(())
    }

    /// Process messages on a certain topic.
    ///
    /// If `signed=true` the messages are expected to be authentic, i.e. they
    /// must be signed by Dria's public key.
    pub async fn process_topic(&self, topic: &str, signed: bool) -> NodeResult<Vec<WakuMessage>> {
        let content_topic = WakuMessage::create_content_topic(topic);
        let mut messages: Vec<WakuMessage> = self.waku.relay.get_messages(&content_topic).await?;

        // dont bother if there are no messages
        if messages.is_empty() {
            return Ok(messages);
        }

        log::debug!("Received {} {} messages.", messages.len(), topic);
        for message in &messages {
            log::debug!("{}", message);
        }

        // if signed, only keep messages that are authentic to Dria
        if signed {
            messages.retain(|message| {
                message
                    .is_signed(&self.config.admin_public_key)
                    .unwrap_or_else(|e| {
                        log::warn!("Could not verify message signature: {}", e);
                        false
                    })
            });
        }

        // sort messages with respect to their timestamp
        messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(messages)
    }

    /// Given a list of messages, this function:
    ///
    /// - parses them into their respective payloads
    /// - checks the signatures (if `signed = true`) w.r.t admin public key
    /// - filters out past-deadline & non-selected (with the Bloom Filter) tasks
    /// - sorts the tasks by their deadline
    pub fn parse_messages<T>(&self, messages: Vec<WakuMessage>, signed: bool) -> Vec<TaskRequest<T>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut task_payloads = messages
            .iter()
            .filter_map(|message| {
                match message.parse_payload::<TaskRequestPayload<T>>(signed) {
                    Ok(task) => {
                        // check if deadline is past or not
                        if get_current_time_nanos() >= task.deadline {
                            log::debug!("Skipping {} due to deadline.", task.task_id);
                            return None;
                        }

                        // check task inclusion via the bloom filter
                        match self.is_tasked(&task.filter) {
                            Ok(is_tasked) => {
                                if !is_tasked {
                                    log::debug!("Skipping {} due to filter.", task.task_id);
                                    return None;
                                }
                            }
                            Err(e) => {
                                log::error!("Error checking task inclusion: {}", e);
                                return None;
                            }
                        }

                        Some(task)
                    }
                    Err(e) => {
                        log::error!("Error parsing payload: {}", e);
                        None
                    }
                }
            })
            .collect::<Vec<TaskRequestPayload<T>>>();

        task_payloads.sort_by(|a, b| a.deadline.cmp(&b.deadline));

        // convert to TaskRequest
        task_payloads
            .into_iter()
            .filter_map(|task| {
                let task_public_key = match hex::decode(&task.public_key) {
                    Ok(public_key) => public_key,
                    Err(e) => {
                        log::error!("Error parsing public key: {}", e);
                        return None;
                    }
                };

                Some(TaskRequest {
                    task_id: task.task_id,
                    input: task.input,
                    public_key: task_public_key,
                })
            })
            .collect()
    }

    /// Given a task with `id` and respective `public_key`, sign-then-encrypt the result.
    pub async fn send_result<R: AsRef<[u8]>>(
        &self,
        response_topic: &str,
        public_key: &[u8],
        task_id: &str,
        result: R,
    ) -> NodeResult<()> {
        let payload = self.create_payload(result.as_ref(), task_id, public_key)?;
        let payload_str = payload.to_string()?;
        let message = WakuMessage::new(payload_str, response_topic);

        self.send_message(message).await
    }
}
