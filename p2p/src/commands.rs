use eyre::{Context, Result};
use libp2p::{gossipsub, kad, request_response, swarm, Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::DriaP2PProtocol;

#[derive(Debug)]
pub enum DriaP2PCommand {
    /// Returns the network information, such as the number of incoming and outgoing connections.
    NetworkInfo {
        sender: oneshot::Sender<swarm::NetworkInfo>,
    },
    /// Make a Kademlia closest-peers query.
    Refresh {
        sender: oneshot::Sender<kad::QueryId>,
    },
    /// Get peers (mesh & all) of the GossipSub pool.
    Peers {
        sender: oneshot::Sender<(Vec<PeerId>, Vec<PeerId>)>,
    },
    /// Get peers counts (mesh & all) of the GossipSub pool.
    PeerCounts {
        sender: oneshot::Sender<(usize, usize)>,
    },
    /// Dial a peer.
    Dial {
        peer_id: Multiaddr,
        sender: oneshot::Sender<Result<(), swarm::DialError>>,
    },
    /// Subscribe to a topic.
    Subscribe {
        topic: String,
        sender: oneshot::Sender<Result<bool, gossipsub::SubscriptionError>>,
    },
    /// Unsubscribe from a topic.
    Unsubscribe {
        topic: String,
        sender: oneshot::Sender<bool>,
    },
    /// Publishes a message to a topic, returns the message ID.
    Publish {
        topic: String,
        data: Vec<u8>,
        sender: oneshot::Sender<Result<gossipsub::MessageId, gossipsub::PublishError>>,
    },
    /// Respond to a request-response message.
    Respond {
        data: Vec<u8>,
        channel: request_response::ResponseChannel<Vec<u8>>,
        sender: oneshot::Sender<Result<()>>,
    },
    /// Request a request-response message.
    /// Note that you are likely to be caught by the RPC peer id check,
    /// and your messages will be ignored.
    Request {
        peer_id: PeerId,
        data: Vec<u8>,
        sender: oneshot::Sender<request_response::OutboundRequestId>,
    },
    /// Validates a GossipSub message for propagation, returns whether the message existed in cache.
    ///
    /// - `Accept`: Accept the message and propagate it.
    /// - `Reject`: Reject the message and do not propagate it, with penalty to `propagation_source`.
    /// - `Ignore`: Ignore the message and do not propagate it, without any penalties.
    ///
    /// See [`validate_messages`](https://docs.rs/libp2p-gossipsub/latest/libp2p_gossipsub/struct.Config.html#method.validate_messages)
    /// and [`report_message_validation_result`](https://docs.rs/libp2p-gossipsub/latest/libp2p_gossipsub/struct.Behaviour.html#method.report_message_validation_result) for more details.
    ValidateMessage {
        msg_id: gossipsub::MessageId,
        propagation_source: PeerId,
        acceptance: gossipsub::MessageAcceptance,
        sender: oneshot::Sender<bool>,
    },
    /// Shutsdown the client, closes the command channel.
    Shutdown { sender: oneshot::Sender<()> },
}

pub struct DriaP2PCommander {
    sender: mpsc::Sender<DriaP2PCommand>,
    protocol: DriaP2PProtocol,
}

impl DriaP2PCommander {
    pub fn new(sender: mpsc::Sender<DriaP2PCommand>, protocol: DriaP2PProtocol) -> Self {
        Self { sender, protocol }
    }

    /// Returns a reference to the protocol.
    pub fn protocol(&self) -> &DriaP2PProtocol {
        &self.protocol
    }

    /// Returns the network information, such as the number of
    /// incoming and outgoing connections.
    pub async fn network_info(&self) -> Result<swarm::NetworkInfo> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::NetworkInfo { sender })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }

    /// Subscribe to a topic.
    pub async fn subscribe(&self, topic_name: &str) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();

        log::debug!("Subscribing to {}", topic_name);
        self.sender
            .send(DriaP2PCommand::Subscribe {
                topic: topic_name.to_string(),
                sender,
            })
            .await
            .wrap_err("could not send")?;

        receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not subscribe")
    }

    /// Unsubscribe from a topic.
    pub async fn unsubscribe(&self, topic_name: &str) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();

        log::debug!("Unsubscribing from {}", topic_name);
        self.sender
            .send(DriaP2PCommand::Unsubscribe {
                topic: topic_name.to_string(),
                sender,
            })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }

    /// Publish a message to a topic.
    ///
    /// Returns the message ID.
    pub async fn publish(
        &mut self,
        topic_name: &str,
        data: Vec<u8>,
    ) -> Result<gossipsub::MessageId> {
        let (sender, receiver) = oneshot::channel();

        log::debug!("Publishing message to topic: {}", topic_name);
        self.sender
            .send(DriaP2PCommand::Publish {
                topic: topic_name.to_string(),
                data,
                sender,
            })
            .await
            .wrap_err("could not send")?;

        receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not publish")
    }

    pub async fn respond(
        &mut self,
        data: Vec<u8>,
        channel: request_response::ResponseChannel<Vec<u8>>,
    ) -> Result<()> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Respond {
                data,
                channel,
                sender,
            })
            .await
            .wrap_err("could not send")?;

        receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not respond")
    }

    pub async fn request(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
    ) -> Result<request_response::OutboundRequestId> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Request {
                data,
                peer_id,
                sender,
            })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }

    /// Dials a given peer.
    pub async fn dial(&mut self, peer_id: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Dial { peer_id, sender })
            .await
            .wrap_err("could not send")?;

        receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not dial")
    }

    /// Validates a GossipSub message for propagation.
    ///
    /// - `Accept`: Accept the message and propagate it.
    /// - `Reject`: Reject the message and do not propagate it, with penalty to `propagation_source`.
    /// - `Ignore`: Ignore the message and do not propagate it, without any penalties.
    ///
    /// See [`validate_messages`](https://docs.rs/libp2p-gossipsub/latest/libp2p_gossipsub/struct.Config.html#method.validate_messages)
    /// and [`report_message_validation_result`](https://docs.rs/libp2p-gossipsub/latest/libp2p_gossipsub/struct.Behaviour.html#method.report_message_validation_result) for more details.
    pub async fn validate_message(
        &mut self,
        msg_id: &gossipsub::MessageId,
        propagation_source: &PeerId,
        acceptance: gossipsub::MessageAcceptance,
    ) -> Result<()> {
        let (sender, receiver) = oneshot::channel();

        log::trace!("Validating message ({}): {:?}", msg_id, acceptance);
        self.sender
            .send(DriaP2PCommand::ValidateMessage {
                msg_id: msg_id.clone(),
                propagation_source: *propagation_source,
                acceptance,
                sender,
            })
            .await
            .wrap_err("could not send")?;

        let msg_was_in_cache = receiver.await.wrap_err("could not receive")?;

        if !msg_was_in_cache {
            log::debug!("Validated message was not in cache.");
        }

        Ok(())
    }

    /// Refreshes the Kademlia DHT using a closest peer query over a random peer.
    pub async fn refresh(&mut self) -> Result<kad::QueryId> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Refresh { sender })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }

    /// Get peers (mesh & all) of the GossipSub pool.
    /// Returns a tuple of the mesh peers and all peers.
    pub async fn peers(&self) -> Result<(Vec<PeerId>, Vec<PeerId>)> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Peers { sender })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }

    /// Get peers counts (mesh & all) of the GossipSub pool.
    /// Returns a tuple of the mesh peers count and all peers count.
    pub async fn peer_counts(&self) -> Result<(usize, usize)> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::PeerCounts { sender })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }

    /// Sends a shutdown signal to the client.
    pub async fn shutdown(&mut self) -> Result<()> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Shutdown { sender })
            .await
            .wrap_err("could not send")?;

        receiver.await.wrap_err("could not receive")
    }
}
