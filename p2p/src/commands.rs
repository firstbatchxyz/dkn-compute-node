use eyre::{Context, Result};
use libp2p::{gossipsub, swarm, PeerId};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub enum DriaP2PCommand {
    /// Returns the network information, such as the number of incoming and outgoing connections.
    NetworkInfo {
        sender: oneshot::Sender<swarm::NetworkInfo>,
    },
    /// Dial a peer.
    Dial {
        peer_id: PeerId,
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
        sender: oneshot::Sender<Result<bool, gossipsub::PublishError>>,
    },
    /// Publishes a message to a topic, returns the message ID.
    Publish {
        topic: String,
        data: Vec<u8>,
        sender: oneshot::Sender<Result<gossipsub::MessageId, gossipsub::PublishError>>,
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
        sender: oneshot::Sender<Result<bool, gossipsub::PublishError>>,
    },
}

pub struct DriaP2PCommander {
    sender: mpsc::Sender<DriaP2PCommand>,
}

impl DriaP2PCommander {
    pub fn new(sender: mpsc::Sender<DriaP2PCommand>) -> Self {
        Self { sender }
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

        receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not unsubscribe")
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

    /// Dials a given peer.
    pub async fn dial(&mut self, peer_id: PeerId) -> Result<()> {
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
                propagation_source: propagation_source.clone(),
                acceptance,
                sender,
            })
            .await
            .wrap_err("could not send")?;

        let msg_was_in_cache = receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not unsubscribe")?;

        if !msg_was_in_cache {
            log::debug!("Validated message was not in cache.");
        }

        Ok(())
    }
}
