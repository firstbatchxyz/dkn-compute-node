use ecies::encrypt;
use fastbloom_rs::{BloomFilter, Membership};
use libp2p::gossipsub;
use libsecp256k1::{sign, Message, RecoveryId, Signature};
use ollama_workflows::ModelProvider;
use serde::Deserialize;
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;

use crate::{
    config::DriaComputeNodeConfig,
    errors::NodeResult,
    handlers::{heartbeat::HandlesHeartbeat, workflow::HandlesWorkflow},
    p2p::{P2PClient, P2PMessage},
    utils::{
        crypto::{secret_to_keypair, sha256hash},
        filter::FilterPayload,
        get_current_time_nanos,
        payload::{TaskRequest, TaskRequestPayload, TaskResponsePayload},
        provider::{check_ollama, check_openai},
    },
};

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub p2p: P2PClient,
    pub cancellation: CancellationToken,
}

impl DriaComputeNode {
    pub fn new(
        config: DriaComputeNodeConfig,
        cancellation: CancellationToken,
    ) -> Result<Self, String> {
        let keypair = secret_to_keypair(&config.secret_key);
        let p2p = P2PClient::new(keypair)?;

        Ok(DriaComputeNode {
            config,
            p2p,
            cancellation,
        })
    }

    /// Shorthand to sign a digest with node's secret key and return signature & recovery id.
    #[inline]
    pub fn sign(&self, message: &Message) -> (Signature, RecoveryId) {
        sign(message, &self.config.secret_key)
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
        task_id: &str,
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
            task_id: task_id.to_string(),
        })
    }

    /// Subscribe to a certain task with its topic.
    pub fn subscribe(&mut self, topic: &str) -> NodeResult<()> {
        let ok = self.p2p.subscribe(topic)?;
        if ok {
            log::info!("Subscribed to {}", topic);
        } else {
            log::info!("Already subscribed to {}", topic);
        }
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic.
    pub fn unsubscribe(&mut self, topic: &str) -> NodeResult<()> {
        let ok = self.p2p.unsubscribe(&topic)?;
        if ok {
            log::info!("Unsubscribed from {}", topic);
        } else {
            log::info!("Already unsubscribed from {}", topic);
        }
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic, ignoring the error.
    pub fn unsubscribe_ignored(&mut self, topic: &str) {
        if let Err(e) = self.unsubscribe(topic) {
            log::error!(
                "Error unsubscribing from {}: {}\nContinuing anyway.",
                topic,
                e
            );
        }
    }

    /// Check if the required compute services are running, e.g. if Ollama
    /// is detected as a provider for the chosen models, it will check that
    /// Ollama is running.
    pub async fn check_services(&self) -> NodeResult<()> {
        let unique_providers: Vec<ModelProvider> =
            self.config
                .models
                .iter()
                .fold(Vec::new(), |mut unique, (provider, _)| {
                    if !unique.contains(provider) {
                        unique.push(provider.clone());
                    }
                    unique
                });

        if unique_providers.contains(&ModelProvider::Ollama) {
            check_ollama().await?;
        }

        if unique_providers.contains(&ModelProvider::OpenAI) {
            check_openai()?;
        }

        Ok(())
    }

    pub fn publish(&mut self, message: P2PMessage) -> NodeResult<()> {
        let message_bytes = message.payload.as_bytes().to_vec();
        self.p2p.publish(&message.topic, message_bytes)?;
        Ok(())
    }

    #[deprecated = "not used anymore"]
    pub fn send_message_once(&mut self, message: P2PMessage) -> NodeResult<()> {
        let topic = message.topic.clone();
        self.subscribe(&topic)?;
        self.publish(message)?;
        self.unsubscribe(&topic)?;
        Ok(())
    }

    /// Launches the main loop of the compute node. This method is not expected to return until cancellation occurs.
    pub async fn launch(&mut self) -> NodeResult<()> {
        const HEARTBEAT_LISTEN_TOPIC: &str = "heartbeat";
        const HEARTBEAT_RESPONSE_TOPIC: &str = "pong";
        const WORKFLOW_LISTEN_TOPIC: &str = "task";
        const WORKFLOW_RESPONSE_TOPIC: &str = "results";

        // subscribe to topics
        self.subscribe(HEARTBEAT_LISTEN_TOPIC)?;
        self.subscribe(HEARTBEAT_RESPONSE_TOPIC)?;
        self.subscribe(WORKFLOW_LISTEN_TOPIC)?;
        self.subscribe(WORKFLOW_RESPONSE_TOPIC)?;

        // main loop, listens for message events in particular
        // the underlying p2p client is expected to handle the rest within its own loop
        loop {
            tokio::select! {
                event = self.p2p.process_events(self.cancellation.clone()) => {
                    if let Some((peer_id, message_id, message)) = event {
                        log::debug!(
                            "Received message id {} from {}:\n{}",
                            message_id,
                            peer_id,
                            String::from_utf8_lossy(&message.data),
                        );

                        // first, parse the raw gossipsub message to a prepared message
                        // TODO: change name of this function
                        let message = match self.parse_message_to_topiced_message(message).await {
                            // TODO: refactor this
                            Some(message) => message,
                            None => continue,
                        };

                        // handle message w.r.t topic
                        match message.topic.as_str() {
                            WORKFLOW_LISTEN_TOPIC => {
                                self.handle_workflow(message, WORKFLOW_RESPONSE_TOPIC).await.unwrap_or_else(|e| {
                                    log::error!("Error handling workflow: {}", e);
                                });
                            }
                            HEARTBEAT_LISTEN_TOPIC => {
                                self.handle_heartbeat(message, HEARTBEAT_RESPONSE_TOPIC).unwrap_or_else(|e| {
                                    log::error!("Error handling heartbeat: {}", e);
                                });
                            }
                            topic => {
                                log::warn!("Unhandled topic: {}", topic);
                            }
                        }
                    }
                },
                _ = wait_for_termination(self.cancellation.clone()) => break,
            }
        }

        // unsubscribe from topics
        self.unsubscribe_ignored(HEARTBEAT_LISTEN_TOPIC);
        self.unsubscribe_ignored(HEARTBEAT_RESPONSE_TOPIC);
        self.unsubscribe_ignored(WORKFLOW_LISTEN_TOPIC);
        self.unsubscribe_ignored(WORKFLOW_RESPONSE_TOPIC);

        Ok(())
    }

    /// Process messages on a certain topic.
    pub async fn parse_message_to_topiced_message(
        &self,
        message: gossipsub::Message,
    ) -> Option<P2PMessage> {
        // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
        log::debug!("Parsing {} message.", message.topic.as_str());
        let message = match P2PMessage::try_from(message) {
            Ok(message) => message,
            Err(e) => {
                log::error!("Could not parse message: {}", e);
                return None;
            }
        };

        // check dria signature
        // TODO: when we have many public keys, we should check the signature against all of them
        if !message
            .is_signed(&self.config.admin_public_key)
            .unwrap_or_else(|e| {
                log::error!("Could not check signature: {}", e);
                false
            })
        {
            log::warn!("Skipping due to invalid signature.");
            return None;
        }

        Some(message)
    }

    pub fn parse_topiced_message_to_task_request<T>(
        &self,
        message: P2PMessage,
    ) -> Option<TaskRequest<T>>
    where
        T: for<'a> Deserialize<'a>,
    {
        // TODO: can remove `true` param here
        let task = match message.parse_payload::<TaskRequestPayload<T>>(true) {
            Ok(task) => task,
            Err(err) => {
                log::error!("Could not parse payload: {}", err);
                return None;
            }
        };

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

        // obtain public key from the payload
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
    }

    /// Given a task with `id` and respective `public_key`, sign-then-encrypt the result.
    pub fn send_result<R: AsRef<[u8]>>(
        &mut self,
        response_topic: &str,
        public_key: &[u8],
        task_id: &str,
        result: R,
    ) -> NodeResult<()> {
        let payload = self.create_payload(result.as_ref(), task_id, public_key)?;
        let payload_str = payload.to_string()?;
        let message = P2PMessage::new(payload_str, response_topic);

        self.publish(message)
    }
}

/// Waits for SIGTERM or SIGINT, and cancels the given token when the signal is received.
async fn wait_for_termination(cancellation: CancellationToken) -> std::io::Result<()> {
    let mut sigterm = signal(SignalKind::terminate())?; // Docker sends SIGTERM
    let mut sigint = signal(SignalKind::interrupt())?; // Ctrl+C sends SIGINT
    tokio::select! {
        _ = sigterm.recv() => log::warn!("Recieved SIGTERM"),
        _ = sigint.recv() => log::warn!("Recieved SIGINT"),
        _ = cancellation.cancelled() => {
            // no need to wait if cancelled anyways
            return Ok(());
        }
    };

    log::info!("Terminating the node...");
    cancellation.cancel();
    Ok(())
}
