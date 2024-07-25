use std::str::FromStr;

use libp2p::{gossipsub, Multiaddr};
use serde::Deserialize;
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;

use crate::{
    config::DriaComputeNodeConfig,
    errors::NodeResult,
    handlers::{HandlesPingpong, HandlesWorkflow},
    p2p::{P2PClient, P2PMessage},
    utils::{
        crypto::secret_to_keypair,
        get_current_time_nanos,
        payload::{TaskRequest, TaskRequestPayload},
    },
};

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub p2p: P2PClient,
    pub cancellation: CancellationToken,
}

impl DriaComputeNode {
    /// Create a new compute node with the given configuration and cancellation token.
    ///
    /// Internally, the node will create a new P2P client with the given secret key.
    /// This P2P client, although created synchronously, requires a tokio runtime.
    ///
    /// ### Example
    ///
    /// ```rs
    /// let config = DriaComputeNodeConfig::new();
    /// let mut node = DriaComputeNode::new(config, CancellationToken::new())?;
    /// node.check_services().await?;
    /// node.launch().await?;
    /// ```
    pub fn new(
        config: DriaComputeNodeConfig,
        cancellation: CancellationToken,
    ) -> Result<Self, String> {
        let keypair = secret_to_keypair(&config.secret_key);
        let listen_addr =
            Multiaddr::from_str(config.p2p_listen_addr.as_str()).map_err(|e| e.to_string())?;
        let p2p = P2PClient::new(keypair, listen_addr, cancellation.clone())?;

        Ok(DriaComputeNode {
            config,
            p2p,
            cancellation,
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
        let ok = self.p2p.unsubscribe(topic)?;
        if ok {
            log::info!("Unsubscribed from {}", topic);
        } else {
            log::info!("Already unsubscribed from {}", topic);
        }
        Ok(())
    }

    /// Publishes a given message to the network.
    /// The topic is expected to be provided within the message struct.
    pub fn publish(&mut self, message: P2PMessage) -> NodeResult<()> {
        let message_bytes = message.payload.as_bytes().to_vec();
        let message_id = self.p2p.publish(&message.topic, message_bytes)?;
        log::info!("Published message ({}) to {}", message_id, message.topic);
        Ok(())
    }

    /// Returns the list of connected peers.
    pub fn peers(&self) -> Vec<(&libp2p_identity::PeerId, Vec<&gossipsub::TopicHash>)> {
        self.p2p.peers()
    }

    /// Launches the main loop of the compute node.
    /// This method is not expected to return until cancellation occurs.
    pub async fn launch(&mut self) -> NodeResult<()> {
        const PINGPONG_LISTEN_TOPIC: &str = "ping";
        const PINGPONG_RESPONSE_TOPIC: &str = "pong";
        const WORKFLOW_LISTEN_TOPIC: &str = "task";
        const WORKFLOW_RESPONSE_TOPIC: &str = "results";

        // subscribe to topics
        self.subscribe(PINGPONG_LISTEN_TOPIC)?;
        self.subscribe(PINGPONG_RESPONSE_TOPIC)?;
        self.subscribe(WORKFLOW_LISTEN_TOPIC)?;
        self.subscribe(WORKFLOW_RESPONSE_TOPIC)?;

        // main loop, listens for message events in particular
        // the underlying p2p client is expected to handle the rest within its own loop
        loop {
            tokio::select! {
                event = self.p2p.process_events() => {
                    if let Some((peer_id, message_id, message)) = event {
                        let topic = message.topic.clone();
                        let topic_str = topic.as_str();

                        // handle message w.r.t topic
                        if std::matches!(topic_str, PINGPONG_LISTEN_TOPIC | WORKFLOW_LISTEN_TOPIC) {
                            log::info!(
                                "Received {} message ({})\nFrom:   {}\nOrigin: {}",
                                topic_str,
                                message_id,
                                peer_id,
                                message.source.map(|p| p.to_string()).unwrap_or("None".to_string())
                            );

                            // first, parse the raw gossipsub message to a prepared message
                            // FIXME: maybe we can avoid the clone here, its done to print data below
                            let message = match self.parse_message_to_prepared_message(message.clone()) {
                                Ok(message) => message,
                                Err(e) => {
                                    log::error!("Error parsing message: {}", e);
                                    log::debug!("Message: {}", String::from_utf8_lossy(&message.data));
                                    continue;
                                }
                            };

                            // then handle the prepared message
                            if let Err(err) = match topic_str {
                                WORKFLOW_LISTEN_TOPIC => {
                                    self.handle_workflow(message, WORKFLOW_RESPONSE_TOPIC).await
                                }
                                PINGPONG_LISTEN_TOPIC => {
                                    self.handle_heartbeat(message, PINGPONG_RESPONSE_TOPIC)
                                }
                                // TODO: can we do this in a nicer way?
                                _ => unreachable!() // unreachable because of the if condition
                            } {
                                log::error!("Error handling {} message: {}", topic_str, err);
                            }
                        } else if std::matches!(topic_str, PINGPONG_RESPONSE_TOPIC | WORKFLOW_RESPONSE_TOPIC) {
                            // since we are responding to these topics, we might receive messages from other compute nodes
                            // we can gracefully ignore them
                            log::trace!("Ignoring message for topic: {}", topic_str);
                        } else {
                            log::warn!("Received unexpected message from topic: {}", topic_str);
                        }

                    }
                },
                _ = wait_for_termination(self.cancellation.clone()) => break,
            }
        }

        // unsubscribe from topics
        self.unsubscribe(PINGPONG_LISTEN_TOPIC)?;
        self.unsubscribe(PINGPONG_RESPONSE_TOPIC)?;
        self.unsubscribe(WORKFLOW_LISTEN_TOPIC)?;
        self.unsubscribe(WORKFLOW_RESPONSE_TOPIC)?;

        Ok(())
    }

    /// Parses a given raw Gossipsub message to a prepared P2PMessage object.
    /// This prepared message includes the topic, payload, version and timestamp.
    pub fn parse_message_to_prepared_message(
        &self,
        message: gossipsub::Message,
    ) -> NodeResult<P2PMessage> {
        // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
        log::debug!("Parsing {} message.", message.topic.as_str());
        let message = P2PMessage::try_from(message)?;
        log::debug!("Parsed: {}", message);

        // check dria signature
        // NOTE: when we have many public keys, we should check the signature against all of them
        if !message.is_signed(&self.config.admin_public_key)? {
            return Err("Invalid signature.".into());
        }

        Ok(message)
    }

    pub fn parse_topiced_message_to_task_request<T>(
        &self,
        message: P2PMessage,
    ) -> NodeResult<Option<TaskRequest<T>>>
    where
        T: for<'a> Deserialize<'a>,
    {
        let task = message.parse_payload::<TaskRequestPayload<T>>(true)?;

        // check if deadline is past or not
        if get_current_time_nanos() >= task.deadline {
            log::info!("Task {} is past the deadline, ignoring.", task.task_id);
            return Ok(None);
        }

        // check task inclusion via the bloom filter
        if !task.filter.contains(&self.config.address)? {
            log::info!(
                "Task {} does not include this node within the filter.",
                task.task_id
            );
            return Ok(None);
        }

        // obtain public key from the payload
        let task_public_key = hex::decode(&task.public_key)?;

        Ok(Some(TaskRequest {
            task_id: task.task_id,
            input: task.input,
            public_key: task_public_key,
        }))
    }

    /// Given a task with `id` and respective `public_key`, sign-then-encrypt the result.
    pub fn send_result<R: AsRef<[u8]>>(
        &mut self,
        response_topic: &str,
        public_key: &[u8],
        task_id: &str,
        result: R,
    ) -> NodeResult<()> {
        let payload = P2PMessage::new_signed_encrypted_payload(
            result.as_ref(),
            task_id,
            public_key,
            &self.config.secret_key,
        )?;
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

#[cfg(test)]
mod tests {
    use crate::{p2p::P2PMessage, DriaComputeNode, DriaComputeNodeConfig};
    use std::env;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_publish_message() {
        env::set_var("RUST_LOG", "info");
        let _ = env_logger::try_init();

        // create node
        let cancellation = CancellationToken::new();
        let mut node = DriaComputeNode::new(DriaComputeNodeConfig::default(), cancellation.clone())
            .expect("should create node");

        // launch & wait for a while for connections
        log::info!("Waiting a bit for peer setup.");
        tokio::select! {
            _ = node.launch() => (),
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(20)) => cancellation.cancel(),
        }
        log::info!("Connected Peers:\n{:#?}", node.peers());

        // publish a dummy message
        let topic = "foo";
        let message = P2PMessage::new("hello from the other side", topic);
        node.subscribe(topic).expect("should subscribe");
        node.publish(message).expect("should publish");
        node.unsubscribe(topic).expect("should unsubscribe");
    }
}
