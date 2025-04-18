use eyre::{Context, Result};
use libp2p::{request_response, swarm, Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::DriaP2PProtocol;

#[derive(Debug)]
pub enum DriaP2PCommand {
    /// Returns the network information, such as the number of incoming and outgoing connections.
    NetworkInfo {
        sender: oneshot::Sender<swarm::NetworkInfo>,
    },
    /// Check if there is an active connection to the given peer.
    IsConnected {
        peer_id: PeerId,
        sender: oneshot::Sender<bool>,
    },
    /// Dial a known peer.
    Dial {
        peer_id: PeerId,
        address: Multiaddr,
        sender: oneshot::Sender<Result<(), swarm::DialError>>,
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
        data: impl Into<Vec<u8>>,
    ) -> Result<request_response::OutboundRequestId> {
        let data = data.into();
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
    pub async fn dial(&mut self, peer_id: PeerId, address: Multiaddr) -> Result<()> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::Dial {
                peer_id,
                address,
                sender,
            })
            .await
            .wrap_err("could not send")?;

        receiver
            .await
            .wrap_err("could not receive")?
            .wrap_err("could not dial")
    }

    /// Checks if there is an active connection to the given peer.
    pub async fn is_connected(&mut self, peer_id: PeerId) -> Result<bool> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(DriaP2PCommand::IsConnected { peer_id, sender })
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
