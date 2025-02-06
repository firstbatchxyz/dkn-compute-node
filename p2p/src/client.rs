use eyre::Result;
use libp2p::futures::StreamExt;
use libp2p::gossipsub::{Message, MessageId};
use libp2p::kad::{GetClosestPeersError, GetClosestPeersOk, QueryResult};
use libp2p::request_response::{self, ResponseChannel};
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::SwarmEvent;
use libp2p::{autonat, gossipsub, identify, kad, multiaddr::Protocol, noise, tcp, yamux};
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
    /// Your peer id.
    pub peer_id: PeerId,
    /// `Swarm` instance, everything p2p-related are accessed through this instace.
    swarm: Swarm<DriaBehaviour>,
    /// Dria protocol, used for identifying the client.
    protocol: DriaP2PProtocol,
    /// Gossipsub protoocol, gossip message sender.
    msg_tx: mpsc::Sender<(PeerId, MessageId, Message)>,
    /// Request-response protocol, request sender.
    req_tx: mpsc::Sender<(PeerId, Vec<u8>, ResponseChannel<Vec<u8>>)>,
    /// Command receiver.
    cmd_rx: mpsc::Receiver<DriaP2PCommand>,
}

// TODO: make all these configurable
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
    #[allow(clippy::type_complexity)]
    pub fn new(
        keypair: Keypair,
        listen_addr: Multiaddr,
        nodes: &DriaNodes,
        protocol: DriaP2PProtocol,
    ) -> Result<(
        DriaP2PClient,
        DriaP2PCommander,
        mpsc::Receiver<(PeerId, MessageId, Message)>,
        mpsc::Receiver<(PeerId, Vec<u8>, ResponseChannel<Vec<u8>>)>,
    )> {
        // this is our peerId
        let peer_id = keypair.public().to_peer_id();
        log::info!("Compute node peer address: {}", peer_id);

        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_relay_client(noise::Config::new, yamux::Config::default)?
            .with_behaviour(|key, relay_behaviour| {
                DriaBehaviour::new(
                    key,
                    relay_behaviour,
                    protocol.identity(),
                    protocol.kademlia(),
                    protocol.request_response(),
                )
                .map_err(Into::into)
            })?
            .with_swarm_config(|c| {
                c.with_idle_connection_timeout(Duration::from_secs(IDLE_CONNECTION_TIMEOUT_SECS))
            })
            .build();

        // set mode to server so that RPC nodes add us to the DHT
        swarm
            .behaviour_mut()
            .kademlia
            .set_mode(Some(libp2p::kad::Mode::Server));

        // initiate bootstrap
        for addr in &nodes.bootstrap_nodes {
            log::info!("Dialling bootstrap: {:#?}", addr);
            if let Some(peer_id) = addr.iter().find_map(|p| match p {
                Protocol::P2p(peer_id) => Some(peer_id),
                _ => None,
            }) {
                swarm.dial(addr.clone())?;
                log::info!("Adding {} to Kademlia routing table", addr);
                swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, addr.clone());
            } else {
                log::warn!("Missing peerID in address: {}", addr);
            }
        }

        // do a random-walk on the DHT with a random peer
        log::info!("Bootstrapping Kademlia DHT.");
        let random_peer = PeerId::random();
        swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(random_peer);
        swarm.behaviour_mut().kademlia.bootstrap()?;

        // listen on all interfaces for incoming connections
        log::info!("Listening p2p network on: {}", listen_addr);
        swarm.listen_on(listen_addr)?;

        // listen on relay addresses with p2p circuit
        for addr in &nodes.relay_nodes {
            log::info!("Listening to relay: {}", addr);
            swarm.listen_on(addr.clone().with(Protocol::P2pCircuit))?;
        }

        // dial rpc nodes
        for rpc_addr in &nodes.rpc_nodes {
            log::info!("Dialing RPC node: {}", rpc_addr);
            if let Err(e) = swarm.dial(rpc_addr.clone()) {
                log::error!("Error dialing RPC node: {:?}", e);
            };
        }

        // create commander
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CHANNEL_BUFSIZE);
        let commander = DriaP2PCommander::new(cmd_tx, protocol.clone());

        // create p2p client itself
        let (msg_tx, msg_rx) = mpsc::channel(MSG_CHANNEL_BUFSIZE);
        let (req_tx, req_rx) = mpsc::channel(MSG_CHANNEL_BUFSIZE);
        let client = Self {
            peer_id,
            swarm,
            protocol,
            msg_tx,
            req_tx,
            cmd_rx,
        };

        Ok((client, commander, msg_rx, req_rx))
    }

    /// Waits for swarm events and Node commands at the same time.
    ///
    /// To terminate, the command channel must be closed.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // this is a special keyword that changes the polling order from random to linear,
                // which will effectively prioritize commands over events
                biased;
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
            DriaP2PCommand::Subscribe { topic, sender } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .subscribe(&gossipsub::IdentTopic::new(topic)),
                );
            }
            DriaP2PCommand::Unsubscribe { topic, sender } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .unsubscribe(&gossipsub::IdentTopic::new(topic))
                        .unwrap_or_default(), // FIXME: due to v0.54 vs 0.55,
                );
            }
            DriaP2PCommand::Publish {
                topic,
                data,
                sender,
            } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(gossipsub::IdentTopic::new(topic), data),
                );
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
            DriaP2PCommand::ValidateMessage {
                msg_id,
                propagation_source,
                acceptance,
                sender,
            } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .report_message_validation_result(&msg_id, &propagation_source, acceptance)
                        .unwrap_or_default(), // FIXME: due to v0.54 vs 0.55,
                );
            }
            DriaP2PCommand::Refresh { sender } => {
                let _ = sender.send(
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .get_closest_peers(PeerId::random()),
                );
            }
            DriaP2PCommand::Peers { sender } => {
                let gossipsub = &self.swarm.behaviour().gossipsub;
                let mesh = gossipsub.all_mesh_peers().cloned().collect();
                let all = gossipsub.all_peers().map(|(p, _)| p).cloned().collect();
                let _ = sender.send((mesh, all));
            }
            DriaP2PCommand::PeerCounts { sender } => {
                let mesh = self.swarm.behaviour().gossipsub.all_mesh_peers().count();
                let all = self.swarm.behaviour().gossipsub.all_peers().count();
                let _ = sender.send((mesh, all));
            }
            DriaP2PCommand::Shutdown { sender } => {
                // close the command channel
                self.cmd_rx.close();

                // remove own peerId from Kademlia DHT
                let peer_id = *self.swarm.local_peer_id();
                self.swarm.behaviour_mut().kademlia.remove_peer(&peer_id);

                // remove own peerId from Autonat server list
                self.swarm.behaviour_mut().autonat.remove_server(&peer_id);

                let _ = sender.send(());
            }
        }
    }

    /// Handles a single event from the `swarm` stream.
    pub async fn handle_event(&mut self, event: SwarmEvent<DriaBehaviourEvent>) {
        match event {
            // this is the main event we are interested in, it will send the message via channel
            SwarmEvent::Behaviour(DriaBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: peer_id,
                message_id,
                message,
            })) => {
                if let Err(e) = self.msg_tx.send((peer_id, message_id, message)).await {
                    log::error!("Could not send Gossipsub message: {:?}", e);
                }
            }

            // request-response events
            SwarmEvent::Behaviour(DriaBehaviourEvent::RequestResponse(
                request_response::Event::Message { message, peer, .. },
            )) => match message {
                // a request has been made with us as the target, and we should respond
                // using the created `channel`; we simply forward this to the request channel
                request_response::Message::Request {
                    request,
                    channel,
                    request_id,
                } => {
                    if let Err(e) = self.req_tx.send((peer, request, channel)).await {
                        log::error!(
                            "Could not send response for request_id {}: {:?}",
                            request_id,
                            e,
                        );
                    }
                }

                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    // while we support the protocol, we dont really make any requests
                    log::warn!(
                        "Unexpected response message with request_id {}: {:?}",
                        request_id,
                        response
                    );
                }
            },
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

            // kademlia events
            SwarmEvent::Behaviour(DriaBehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result: QueryResult::GetClosestPeers(result),
                    ..
                },
            )) => self.handle_closest_peers_result(result),

            // identify events
            SwarmEvent::Behaviour(DriaBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => self.handle_identify_event(peer_id, info),

            // autonat events
            SwarmEvent::Behaviour(DriaBehaviourEvent::Autonat(autonat::Event::StatusChanged {
                old,
                new,
            })) => {
                log::warn!("AutoNAT status changed from {:?} to {:?}", old, new);
            }

            // log listen addreses
            SwarmEvent::NewListenAddr { address, .. } => {
                log::warn!("Local node is listening on {}", address);
            }

            // add external address of peers to Kademlia routing table
            SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, address);
            }
            // add your own peer_id to kademlia as well
            SwarmEvent::ExternalAddrConfirmed { address } => {
                // this is usually the external address via relay
                log::info!("External address confirmed: {}", address);
                let peer_id = *self.swarm.local_peer_id();
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, address);
            }

            // SwarmEvent::IncomingConnectionError {
            //     local_addr,
            //     send_back_addr,
            //     error,
            //     connection_id,
            // } => {
            //     log::debug!(
            //         "Incoming connection {} error: from {} to {} - {:?}",
            //         connection_id,
            //         local_addr,
            //         send_back_addr,
            //         error
            //     );
            // }

            // SwarmEvent::IncomingConnection {
            //     connection_id,
            //     local_addr,
            //     send_back_addr,
            // } => {
            //     log::debug!(
            //         "Incoming connection {} attepmt: from {} to {}",
            //         connection_id,
            //         local_addr,
            //         send_back_addr
            //     );
            // }

            // SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            //     if let Some(peer_id) = peer_id {
            //         log::warn!("Could not connect to peer {}: {:?}", peer_id, error);
            //     } else {
            //         log::warn!("Outgoing connection error: {:?}", error);
            //     }
            // }
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

            // blacklist & disconnect peers with different protocol
            self.swarm
                .behaviour_mut()
                .gossipsub
                .blacklist_peer(&peer_id);
            let _ = self.swarm.disconnect_peer_id(peer_id);
        } else {
            // check kademlia protocol
            if let Some(kad_protocol) = info
                .protocols
                .iter()
                .find(|p| self.protocol.is_common_kademlia(p))
            {
                // if it matches our protocol, add it to the Kademlia routing table
                if *kad_protocol == self.protocol.kademlia {
                    // filter listen addresses
                    let addrs = info.listen_addrs.into_iter().filter(|listen_addr| {
                        if let Some(Protocol::Ip4(ipv4_addr)) = listen_addr.iter().next() {
                            // ignore private & localhost addresses
                            !(ipv4_addr.is_private() || ipv4_addr.is_loopback())
                        } else {
                            // ignore non ipv4 addresses
                            false
                        }
                    });

                    // add them to kademlia
                    for addr in addrs {
                        log::info!(
                            "Identify: {} peer {} identified at {}",
                            self.protocol.kademlia,
                            peer_id,
                            addr
                        );

                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                } else {
                    log::warn!(
                        "Identify: Peer {} has different Kademlia version: (them {}, you {})",
                        peer_id,
                        kad_protocol,
                        self.protocol.kademlia
                    );

                    // blacklist & disconnect peers with different kademlia protocol
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .blacklist_peer(&peer_id);
                    let _ = self.swarm.disconnect_peer_id(peer_id);
                }
            }
        }
    }

    /// Handles the results of a Kademlia closest peers search, simply logs it.
    fn handle_closest_peers_result(
        &mut self,
        result: Result<GetClosestPeersOk, GetClosestPeersError>,
    ) {
        match result {
            Ok(GetClosestPeersOk { peers, .. }) => {
                log::info!(
                    "Kademlia: Query finished with {} closest peers.",
                    peers.len()
                );
            }
            Err(GetClosestPeersError::Timeout { peers, .. }) => {
                log::info!(
                    "Kademlia: Query timed out with {} closest peers.",
                    peers.len()
                );
            }
        }
    }
}
