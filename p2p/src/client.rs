use eyre::Result;
use libp2p::futures::StreamExt;
use libp2p::swarm::{
    dial_opts::{DialOpts, PeerCondition},
    SwarmEvent,
};
use libp2p::{identify, noise, request_response, tcp, yamux};
use libp2p::{Multiaddr, PeerId, Swarm, SwarmBuilder};
use libp2p_identity::Keypair;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::behaviour::{DriaBehaviour, DriaBehaviourEvent};
use crate::{DriaNodes, DriaP2PProtocol};

use super::commands::DriaP2PCommand;
use super::DriaP2PCommander;

/// Peer-to-peer client for Dria Knowledge Network.
pub struct DriaP2PClient {
    pub peer_id: PeerId,
    /// `Swarm` instance, everything p2p-related are accessed through this instace.
    swarm: Swarm<DriaBehaviour>,
    /// Dria protocol, used for identifying the client.
    protocol: DriaP2PProtocol,
    /// Request-response protocol messages.
    reqres_tx: mpsc::Sender<(PeerId, request_response::Message<Vec<u8>, Vec<u8>>)>,
    /// Command receiver.
    cmd_rx: mpsc::Receiver<DriaP2PCommand>,
}

/// Number of seconds before an idle connection is closed.
const IDLE_CONNECTION_TIMEOUT_SECS: u64 = 60;
/// Buffer size for command channel.
const COMMAND_CHANNEL_BUFSIZE: usize = 1024;
/// Buffer size for events channel.
const MSG_CHANNEL_BUFSIZE: usize = 1024;

impl DriaP2PClient {
    /// Creates a new P2P client with the given keypair and listen address.
    ///
    /// Can provide a list of bootstrap and relay nodes to connect to as well at the start, and RPC addresses to dial preemptively.
    ///
    /// The `version` is used to create the protocol strings for the client, and its very important that
    /// they match with the clients existing within the network.
    ///
    /// If for any reason the given `listen_addr` is not available, it will try to listen on a random port on `localhost`.
    pub fn new(
        keypair: Keypair,
        listen_addr: Multiaddr,
        nodes: &DriaNodes,
        protocol: DriaP2PProtocol,
    ) -> Result<(
        DriaP2PClient,
        DriaP2PCommander,
        mpsc::Receiver<(PeerId, request_response::Message<Vec<u8>, Vec<u8>>)>,
    )> {
        let peer_id = keypair.public().to_peer_id();

        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                DriaBehaviour::new(key, protocol.identity(), protocol.request_response())
                    .expect("TODO: !!!")
            })?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(Duration::from_secs(IDLE_CONNECTION_TIMEOUT_SECS))
            })
            .build();

        // listen on all interfaces for incoming connections
        log::info!("Listening p2p network on: {}", listen_addr);
        if let Err(e) = swarm.listen_on(listen_addr) {
            log::error!("Could not listen on address: {:?}", e);
            log::warn!("Trying fallback address with localhost random port");
            swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())?;
        }

        // dial rpc nodes
        // this will cause `identify` event to be called on their side
        for rpc_addr in nodes.rpc_addrs.iter().cloned() {
            log::info!("Dialing RPC node: {}", rpc_addr);
            if let Err(e) = swarm.dial(rpc_addr) {
                log::error!("Could not dial RPC node: {:?}", e);
            };
        }

        // create commander
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CHANNEL_BUFSIZE);
        let commander = DriaP2PCommander::new(cmd_tx, protocol.clone());

        // create p2p client itself
        let (reqres_tx, reqres_rx) = mpsc::channel(MSG_CHANNEL_BUFSIZE);
        let client = Self {
            peer_id,
            swarm,
            protocol,
            reqres_tx,
            cmd_rx,
        };

        Ok((client, commander, reqres_rx))
    }

    /// Waits for swarm events and Node commands at the same time.
    ///
    /// To terminate, the command channel must be closed.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                command = self.cmd_rx.recv() => match command {
                    Some(c) => self.handle_command(c).await,
                    // channel closed, thus shutting down the network event loop
                    None=>  {
                        log::info!("Closing peer-to-peer client.");
                        return
                    },
                },
                event = self.swarm.select_next_some() => self.handle_event(event).await,
            }
        }
    }

    /// Handles a single command, which originates from `DriaP2PCommander`.
    pub async fn handle_command(&mut self, command: DriaP2PCommand) {
        match command {
            DriaP2PCommand::Dial {
                peer_id,
                address,
                sender,
            } => {
                let opts = DialOpts::peer_id(peer_id)
                    .addresses(vec![address])
                    .condition(PeerCondition::Always)
                    .build();
                let _ = sender.send(self.swarm.dial(opts));
            }
            DriaP2PCommand::NetworkInfo { sender } => {
                let _ = sender.send(self.swarm.network_info());
            }
            DriaP2PCommand::Respond {
                data,
                channel,
                sender,
            } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, data)
                        .map_err(|_| eyre::eyre!("could not send response, channel is closed?")),
                );
            }
            DriaP2PCommand::Request {
                data,
                peer_id,
                sender,
            } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&peer_id, data),
                );
            }
            DriaP2PCommand::Shutdown { sender } => {
                // close the command channel
                self.cmd_rx.close();

                let _ = sender.send(());
            }
        }
    }

    /// Handles a single event from the `swarm` stream.
    pub async fn handle_event(&mut self, event: SwarmEvent<DriaBehaviourEvent>) {
        match event {
            /*****************************************
             * Request-response events               *
             *****************************************/
            SwarmEvent::Behaviour(DriaBehaviourEvent::RequestResponse(
                request_response::Event::Message { message, peer },
            )) => {
                // whether its a request or response, we forward it to the main thread
                if let Err(err) = self.reqres_tx.send((peer, message)).await {
                    log::error!("Could not transfer request {:?}", err);
                }
            }

            SwarmEvent::Behaviour(DriaBehaviourEvent::RequestResponse(
                request_response::Event::ResponseSent {
                    peer, request_id, ..
                },
            )) => {
                log::debug!(
                    "Request-Response: Response sent to peer {} with request_id {}",
                    peer,
                    request_id,
                )
            }
            SwarmEvent::Behaviour(DriaBehaviourEvent::RequestResponse(
                request_response::Event::OutboundFailure {
                    peer,
                    request_id,
                    error,
                    ..
                },
            )) => {
                log::error!(
                    "Request-Response: Outbound failure to peer {} with request_id {}: {:?}",
                    peer,
                    request_id,
                    error
                );
            }
            SwarmEvent::Behaviour(DriaBehaviourEvent::RequestResponse(
                request_response::Event::InboundFailure {
                    peer,
                    request_id,
                    error,
                    ..
                },
            )) => {
                log::error!(
                    "Request-Response: Inbound failure to peer {} with request_id {}: {:?}",
                    peer,
                    request_id,
                    error
                );
            }

            /*****************************************
             * Identify events                       *
             *****************************************/
            SwarmEvent::Behaviour(DriaBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => self.handle_identify_event(peer_id, info),

            // log listen addreses
            SwarmEvent::NewListenAddr { address, .. } => {
                log::warn!("Local node is listening on {}", address);
            }

            // add external address of peers to Kademlia routing table
            SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
                log::info!(
                    "External address of peer {} confirmed: {}",
                    peer_id,
                    address
                );
            }
            // add your own peer_id to kademlia as well
            SwarmEvent::ExternalAddrConfirmed { address } => {
                // this is usually the external address via relay
                log::info!("External address confirmed: {}", address);
            }

            /*****************************************
             * Connection events and errors handling *
             *****************************************/
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                ..
            } => {
                log::debug!(
                    "Incoming connection error: from {} to {} - {:?}",
                    local_addr,
                    send_back_addr,
                    error
                );
            }

            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
                ..
            } => {
                log::debug!(
                    "Incoming connection  attempt: from {} to {}",
                    local_addr,
                    send_back_addr
                );
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    log::warn!("Could not connect to peer {}: {:?}", peer_id, error);
                } else {
                    log::warn!("Outgoing connection error: {:?}", error);
                }
            }

            SwarmEvent::ConnectionEstablished {
                peer_id,
                num_established,
                ..
            } => {
                log::info!(
                    "Connection established with peer {} ({} connections)",
                    peer_id,
                    num_established
                );
            }

            event => log::trace!("Unhandled Swarm Event: {:?}", event),
        }
    }

    /// Handles identify events.
    ///
    /// At the top level, we check the protocol string.
    ///
    /// - For Kademlia, we check the kademlia protocol and then add the address to the Kademlia routing table.
    fn handle_identify_event(&mut self, peer_id: PeerId, info: identify::Info) {
        // check identify protocol string
        if info.protocol_version != self.protocol.identity {
            log::warn!(
                "Identify: Peer {} has different Identify protocol: (them {}, you {})",
                peer_id,
                info.protocol_version,
                self.protocol.identity
            );

            // disconnect them
            let _ = self.swarm.disconnect_peer_id(peer_id);
        }
    }
}
