use dkn_p2p::libp2p::gossipsub::{Message, MessageAcceptance, MessageId};
use dkn_p2p::libp2p::PeerId;
use eyre::Result;

use crate::utils::DriaMessage;
use crate::DriaComputeNode;

use crate::gossipsub::*;

impl DriaComputeNode {
    /// Subscribe to a certain task with its topic.
    ///
    /// These are likely to be called once, so can be inlined.
    #[inline]
    pub async fn subscribe(&mut self, topic: &str) -> Result<()> {
        if self.p2p.subscribe(topic).await? {
            log::info!("Subscribed to {}", topic);
        } else {
            log::info!("Already subscribed to {}", topic);
        }
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic.
    ///
    /// These are likely to be called once, so can be inlined.
    #[inline]
    pub async fn unsubscribe(&mut self, topic: &str) -> Result<()> {
        if self.p2p.unsubscribe(topic).await? {
            log::info!("Unsubscribed from {}", topic);
        } else {
            log::info!("Already unsubscribed from {}", topic);
        }
        Ok(())
    }

    /// Publishes a given message to the network w.r.t the topic of it.
    ///
    /// The entire message is serialized to JSON in bytes and then published.
    pub async fn publish(&mut self, message: DriaMessage) -> Result<()> {
        let message_bytes = serde_json::to_vec(&message)?;
        let message_id = self.p2p.publish(&message.topic, message_bytes).await?;
        log::info!("Published {} message ({})", message.topic, message_id);
        Ok(())
    }

    /// Returns the list of connected peers within GossipSub, `mesh` and `all`.
    #[inline(always)]
    pub async fn peers(&self) -> Result<(Vec<PeerId>, Vec<PeerId>)> {
        self.p2p.peers().await
    }

    /// Handles a GossipSub message received from the network.
    pub(crate) async fn handle_message(
        &mut self,
        (propagation_peer_id, message_id, gossipsub_message): (PeerId, &MessageId, Message),
    ) -> MessageAcceptance {
        // handle message with respect to its topic
        match gossipsub_message.topic.as_str() {
            PingpongHandler::LISTEN_TOPIC => {
                // ensure that the message is from a valid source (origin)
                let Some(source_peer_id) = gossipsub_message.source else {
                    log::warn!(
                        "Received {} message from {} without source.",
                        gossipsub_message.topic,
                        propagation_peer_id
                    );
                    return MessageAcceptance::Ignore;
                };

                // ensure that message is from the known RPCs
                if !self.dria_nodes.rpc_peerids.contains(&source_peer_id) {
                    log::warn!(
                        "Received message from unauthorized source: {}, allowed sources: {:#?}",
                        source_peer_id,
                        self.dria_nodes.rpc_peerids
                    );
                    return MessageAcceptance::Ignore;
                }

                // parse the raw gossipsub message to a prepared DKN message
                // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
                log::debug!("Parsing {} message.", gossipsub_message.topic.as_str());
                let message: DriaMessage = match serde_json::from_slice(&gossipsub_message.data) {
                    Ok(message) => message,
                    Err(e) => {
                        log::error!("Error parsing message: {:?}", e);
                        log::debug!(
                            "Message: {}",
                            String::from_utf8_lossy(&gossipsub_message.data)
                        );
                        return MessageAcceptance::Ignore;
                    }
                };

                // debug-log the received message
                log::debug!(
                    "Received {} message ({}) from {}\n{}",
                    gossipsub_message.topic,
                    message_id,
                    propagation_peer_id,
                    message
                );

                // check signature w.r.t recovered peer id
                match message.is_signed(&self.dria_nodes.rpc_peerids) {
                    Ok(true) => { /* message is signed correctly, nothing to do here */ }
                    Ok(false) => {
                        log::warn!("Message has wrong signature!");
                        return MessageAcceptance::Reject;
                    }
                    Err(e) => {
                        log::error!("Error verifying signature: {:?}", e);
                        return MessageAcceptance::Ignore;
                    }
                }

                // handle the DKN message with respect to the topic
                let handler_result = match gossipsub_message.topic.as_str() {
                    PingpongHandler::LISTEN_TOPIC => {
                        PingpongHandler::handle_ping(self, &message).await
                    }
                    _ => unreachable!("unreachable due to match expression"),
                };

                // validate the message based on the result
                handler_result.unwrap_or_else(|err| {
                    log::error!(
                        "Error handling {} message: {:?}",
                        gossipsub_message.topic,
                        err
                    );
                    MessageAcceptance::Ignore
                })
            }
            PingpongHandler::RESPONSE_TOPIC => {
                // since we are responding to these topics, we might receive messages from other compute nodes
                // we can gracefully ignore them and propagate it to to others
                log::trace!("Ignoring {} message", gossipsub_message.topic);
                MessageAcceptance::Accept
            }
            other => {
                // reject this message as its from a foreign topic
                log::warn!("Received message from unexpected topic: {}", other);
                MessageAcceptance::Reject
            }
        }
    }
}
