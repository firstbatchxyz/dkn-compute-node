use eyre::{eyre, Result};
use libp2p::{gossipsub, Multiaddr};
use std::{str::FromStr, time::Duration};
use tokio_util::sync::CancellationToken;

use crate::{
    config::DriaComputeNodeConfig,
    handlers::{ComputeHandler, PingpongHandler, WorkflowHandler},
    p2p::{P2PClient, P2PMessage},
    utils::{crypto::secret_to_keypair, AvailableNodes},
};

/// Number of seconds between refreshing the Admin RPC PeerIDs from Dria server.
const RPC_PEER_ID_REFRESH_INTERVAL_SECS: u64 = 30;

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub p2p: P2PClient,
    pub available_nodes: AvailableNodes,
    pub available_nodes_last_refreshed: tokio::time::Instant,
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
    pub async fn new(
        config: DriaComputeNodeConfig,
        cancellation: CancellationToken,
    ) -> Result<Self, String> {
        let keypair = secret_to_keypair(&config.secret_key);
        let listen_addr =
            Multiaddr::from_str(config.p2p_listen_addr.as_str()).map_err(|e| e.to_string())?;

        // get available nodes (bootstrap, relay, rpc) for p2p
        let available_nodes = AvailableNodes::default()
            .join(AvailableNodes::new_from_statics())
            .join(AvailableNodes::new_from_env())
            .join(
                AvailableNodes::get_available_nodes()
                    .await
                    .unwrap_or_default(),
            )
            .sort_dedup();

        let p2p = P2PClient::new(keypair, listen_addr, &available_nodes)?;

        Ok(DriaComputeNode {
            config,
            p2p,
            cancellation,
            available_nodes,
            available_nodes_last_refreshed: tokio::time::Instant::now(),
        })
    }

    /// Subscribe to a certain task with its topic.
    pub fn subscribe(&mut self, topic: &str) -> Result<()> {
        let ok = self.p2p.subscribe(topic)?;
        if ok {
            log::info!("Subscribed to {}", topic);
        } else {
            log::info!("Already subscribed to {}", topic);
        }
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic.
    pub fn unsubscribe(&mut self, topic: &str) -> Result<()> {
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
    pub fn publish(&mut self, message: P2PMessage) -> Result<()> {
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
    pub async fn launch(&mut self) -> Result<()> {
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
                    // refresh admin rpc peer ids
                    if self.available_nodes_last_refreshed.elapsed() > Duration::from_secs(RPC_PEER_ID_REFRESH_INTERVAL_SECS) {
                        self.available_nodes = AvailableNodes::get_available_nodes().await.unwrap_or_default().join(self.available_nodes.clone()).sort_dedup();
                        self.available_nodes_last_refreshed = tokio::time::Instant::now();
                    }

                    let (peer_id, message_id, message) = event;
                    let topic = message.topic.clone();
                    let topic_str = topic.as_str();

                    // handle message w.r.t topic
                    if std::matches!(topic_str, PINGPONG_LISTEN_TOPIC | WORKFLOW_LISTEN_TOPIC) {
                        // ensure that the message is from a valid source (origin)
                        let source_peer_id = match message.source {
                            Some(peer) => peer,
                            None => {
                                log::warn!("Received {} message from {} without source.", topic_str, peer_id);
                                self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Ignore)?;
                                continue;
                            }
                        };

                        log::info!(
                            "Received {} message ({})\nFrom:   {}\nSource: {}",
                            topic_str,
                            message_id,
                            peer_id,
                            source_peer_id
                        );

                        // ensure that message is from the static RPCs
                        if !self.available_nodes.rpc_nodes.contains(&source_peer_id) {
                            log::warn!("Received message from unauthorized source: {}", source_peer_id);
                            log::debug!("Allowed sources: {:#?}", self.available_nodes.rpc_nodes);
                            self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Ignore)?;
                            continue;
                        }

                        // first, parse the raw gossipsub message to a prepared message
                        // if unparseable,
                        let message = match self.parse_message_to_prepared_message(message.clone()) {
                            Ok(message) => message,
                            Err(e) => {
                                log::error!("Error parsing message: {}", e);
                                log::debug!("Message: {}", String::from_utf8_lossy(&message.data));
                                self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Ignore)?;
                                continue;
                            }
                        };

                        // then handle the prepared message
                        let handle_result = match topic_str {
                            WORKFLOW_LISTEN_TOPIC => {
                                WorkflowHandler::handle_compute(self, message, WORKFLOW_RESPONSE_TOPIC).await
                            }
                            PINGPONG_LISTEN_TOPIC => {
                                PingpongHandler::handle_compute(self, message, PINGPONG_RESPONSE_TOPIC).await
                            }
                            // TODO: can we do this in a nicer way?
                            // TODO: yes, cast to enum above and let type-casting do the work
                            _ => unreachable!() // unreachable because of the if condition
                        };

                        // validate the message based on the result
                        match handle_result {
                            Ok(acceptance) => {

                                self.p2p.validate_message(&message_id, &peer_id, acceptance)?;
                            },
                            Err(err) => {
                                log::error!("Error handling {} message: {}", topic_str, err);
                                self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Reject)?;
                            }
                        }
                    } else if std::matches!(topic_str, PINGPONG_RESPONSE_TOPIC | WORKFLOW_RESPONSE_TOPIC) {
                        // since we are responding to these topics, we might receive messages from other compute nodes
                        // we can gracefully ignore them
                        log::debug!("Ignoring message for topic: {}", topic_str);

                        // accept this message for propagation
                        self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Accept)?;
                    } else {
                        log::warn!("Received message from unexpected topic: {}", topic_str);

                        // reject this message as its from a foreign topic
                        self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Reject)?;
                    }
                },
                _ = self.cancellation.cancelled() => break,
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
    ///
    /// This also checks the signature of the message, expecting a valid signature from admin node.
    pub fn parse_message_to_prepared_message(
        &self,
        message: gossipsub::Message,
    ) -> Result<P2PMessage> {
        // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
        log::debug!("Parsing {} message.", message.topic.as_str());
        let message = P2PMessage::try_from(message)?;
        log::debug!("Parsed: {}", message);

        // check dria signature
        // NOTE: when we have many public keys, we should check the signature against all of them
        if !message.is_signed(&self.config.admin_public_key)? {
            return Err(eyre!("Invalid signature."));
        }

        Ok(message)
    }

    /// Given a task with `id` and respective `public_key`, sign-then-encrypt the result.
    pub fn send_result<R: AsRef<[u8]>>(
        &mut self,
        response_topic: &str,
        public_key: &[u8],
        task_id: &str,
        result: R,
    ) -> Result<()> {
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
            .await
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
