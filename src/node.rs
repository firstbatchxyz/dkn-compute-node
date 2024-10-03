use eyre::{eyre, Result};
use libp2p::gossipsub;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::{
    config::*,
    handlers::*,
    p2p::P2PClient,
    utils::{crypto::secret_to_keypair, AvailableNodes, DKNMessage},
};

/// Number of seconds between refreshing the Admin RPC PeerIDs from Dria server.
const RPC_PEER_ID_REFRESH_INTERVAL_SECS: u64 = 30;

/// **Dria Compute Node**
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
pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    pub p2p: P2PClient,
    pub available_nodes: AvailableNodes,
    pub available_nodes_last_refreshed: tokio::time::Instant,
    pub cancellation: CancellationToken,
}

impl DriaComputeNode {
    pub async fn new(
        config: DriaComputeNodeConfig,
        cancellation: CancellationToken,
    ) -> Result<Self> {
        let keypair = secret_to_keypair(&config.secret_key);

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

        let p2p = P2PClient::new(keypair, config.p2p_listen_addr.clone(), &available_nodes)?;

        Ok(DriaComputeNode {
            p2p,
            config,
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

    /// Publishes a given message to the network w.r.t the topic of it.
    ///
    /// Internally, the message is JSON serialized to bytes and then published to the network as is.
    pub fn publish(&mut self, message: DKNMessage) -> Result<()> {
        let message_bytes = serde_json::to_vec(&message)?;
        let message_id = self.p2p.publish(&message.topic, message_bytes)?;
        log::info!("Published message ({}) to {}", message_id, message.topic);
        Ok(())
    }

    /// Returns the list of connected peers.
    #[inline(always)]
    pub fn peers(&self) -> Vec<(&libp2p_identity::PeerId, Vec<&gossipsub::TopicHash>)> {
        self.p2p.peers()
    }

    /// Launches the main loop of the compute node.
    /// This method is not expected to return until cancellation occurs.
    pub async fn launch(&mut self) -> Result<()> {
        // subscribe to topics
        self.subscribe(PingpongHandler::LISTEN_TOPIC)?;
        self.subscribe(PingpongHandler::RESPONSE_TOPIC)?;
        self.subscribe(WorkflowHandler::LISTEN_TOPIC)?;
        self.subscribe(WorkflowHandler::RESPONSE_TOPIC)?;

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
                    if std::matches!(topic_str, PingpongHandler::LISTEN_TOPIC | WorkflowHandler::LISTEN_TOPIC) {
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
                                log::error!("Error parsing message: {:?}", e);
                                log::debug!("Message: {}", String::from_utf8_lossy(&message.data));
                                self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Ignore)?;
                                continue;
                            }
                        };

                        // then handle the prepared message
                        let handler_result = match topic_str {
                            WorkflowHandler::LISTEN_TOPIC => {
                                WorkflowHandler::handle_compute(self, message).await
                            }
                            PingpongHandler::LISTEN_TOPIC => {
                                PingpongHandler::handle_compute(self, message).await
                            }
                            // TODO: can we do this in a nicer way?
                            // TODO: yes, cast to enum above and let type-casting do the work
                            _ => unreachable!() // unreachable because of the if condition
                        };

                        // validate the message based on the result
                        match handler_result {
                            Ok(acceptance) => {
                                self.p2p.validate_message(&message_id, &peer_id, acceptance)?;
                            },
                            Err(err) => {
                                log::error!("Error handling {} message: {:?}", topic_str, err);
                                self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Ignore)?;
                            }
                        }
                    } else if std::matches!(topic_str, PingpongHandler::RESPONSE_TOPIC | WorkflowHandler::RESPONSE_TOPIC) {
                        // since we are responding to these topics, we might receive messages from other compute nodes
                        // we can gracefully ignore them and propagate it to to others
                        log::debug!("Ignoring message for topic: {}", topic_str);
                        self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Accept)?;
                    } else {
                        // reject this message as its from a foreign topic
                        log::warn!("Received message from unexpected topic: {}", topic_str);
                        self.p2p.validate_message(&message_id, &peer_id, gossipsub::MessageAcceptance::Reject)?;
                    }
                },
                _ = self.cancellation.cancelled() => break,
            }
        }

        // unsubscribe from topics
        self.unsubscribe(PingpongHandler::LISTEN_TOPIC)?;
        self.unsubscribe(PingpongHandler::RESPONSE_TOPIC)?;
        self.unsubscribe(WorkflowHandler::LISTEN_TOPIC)?;
        self.unsubscribe(WorkflowHandler::RESPONSE_TOPIC)?;

        Ok(())
    }

    /// Parses a given raw Gossipsub message to a prepared P2PMessage object.
    /// This prepared message includes the topic, payload, version and timestamp.
    ///
    /// This also checks the signature of the message, expecting a valid signature from admin node.
    // TODO: move this somewhere?
    pub fn parse_message_to_prepared_message(
        &self,
        message: gossipsub::Message,
    ) -> Result<DKNMessage> {
        // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
        log::debug!("Parsing {} message.", message.topic.as_str());
        let message = DKNMessage::try_from(message)?;
        log::debug!("Parsed: {}", message);

        // check dria signature
        // NOTE: when we have many public keys, we should check the signature against all of them
        // TODO: public key here will be given dynamically
        if !message.is_signed(&self.config.admin_public_key)? {
            return Err(eyre!("Invalid signature."));
        }

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

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
        let message = DKNMessage::new("hello from the other side", topic);
        node.subscribe(topic).expect("should subscribe");
        node.publish(message).expect("should publish");
        node.unsubscribe(topic).expect("should unsubscribe");
    }
}
