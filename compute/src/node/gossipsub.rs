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
        let ok = self.p2p.subscribe(topic).await?;
        if ok {
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
        let ok = self.p2p.unsubscribe(topic).await?;
        if ok {
            log::info!("Unsubscribed from {}", topic);
        } else {
            log::info!("Already unsubscribed from {}", topic);
        }
        Ok(())
    }

    /// Publishes a given message to the network w.r.t the topic of it.
    ///
    /// Internally, identity is attached to the the message which is then JSON serialized to bytes
    /// and then published to the network as is.
    pub async fn publish(&mut self, mut message: DriaMessage) -> Result<()> {
        // attach protocol name to the message
        message = message.with_protocol(self.p2p.protocol());

        let message_bytes = serde_json::to_vec(&message)?;
        let message_id = self.p2p.publish(&message.topic, message_bytes).await?;
        log::info!("Published message ({}) to {}", message_id, message.topic);
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
        (peer_id, message_id, gossipsub_message): (PeerId, &MessageId, Message),
    ) -> MessageAcceptance {
        // handle message with respect to its topic
        match gossipsub_message.topic.as_str() {
            PingpongHandler::LISTEN_TOPIC => {
                // ensure that the message is from a valid source (origin)
                let Some(source_peer_id) = gossipsub_message.source else {
                    log::warn!(
                        "Received {} message from {} without source.",
                        gossipsub_message.topic,
                        peer_id
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
                    peer_id,
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
                let handler_result = match message.topic.as_str() {
                    PingpongHandler::LISTEN_TOPIC => {
                        PingpongHandler::handle_ping(self, &message).await
                    }
                    _ => unreachable!("unreachable due to match expression"),
                };

                // validate the message based on the result
                handler_result.unwrap_or_else(|err| {
                    log::error!("Error handling {} message: {:?}", message.topic, err);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DriaComputeNodeConfig;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_publish_message() -> eyre::Result<()> {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_compute", log::LevelFilter::Debug)
            .filter_module("dkn_p2p", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // create node
        let cancellation = CancellationToken::new();
        let (mut node, p2p, _, _) = DriaComputeNode::new(DriaComputeNodeConfig::default()).await?;

        // spawn p2p task
        let p2p_task = tokio::spawn(async move { p2p.run().await });

        // launch & wait for a while for connections
        log::info!("Waiting a bit for peer setup.");
        let run_cancellation = cancellation.clone();
        tokio::select! {
            _ = node.run(run_cancellation) => (),
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(20)) => cancellation.cancel(),
        }
        log::info!("Connected Peers:\n{:#?}", node.peers().await?);

        // publish a dummy message
        let topic = "foo";
        let message = DriaMessage::new("hello from the other side", topic);
        node.subscribe(topic).await?;
        node.publish(message).await?;
        node.unsubscribe(topic).await?;

        // close everything
        log::info!("Shutting down node.");
        node.p2p.shutdown().await?;

        // wait for task handle
        p2p_task.await?;

        Ok(())
    }
}
