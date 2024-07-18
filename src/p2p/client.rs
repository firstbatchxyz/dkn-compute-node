use libp2p::futures::StreamExt;
use libp2p::gossipsub::{Message, MessageId, PublishError, SubscriptionError, TopicHash};
use libp2p::kad::{GetClosestPeersError, GetClosestPeersOk, QueryResult};
use libp2p::{gossipsub, identify, kad, multiaddr::Protocol, noise, swarm::SwarmEvent, tcp, yamux};
use libp2p::{Multiaddr, PeerId, Swarm, SwarmBuilder};
use libp2p_identity::Keypair;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::utils::split_comma_separated;

use super::{DriaBehaviour, DriaBehaviourEvent, DRIA_PROTO_NAME};

/// Underlying libp2p client.
pub struct P2PClient {
    swarm: Swarm<DriaBehaviour>,
    cancellation: CancellationToken,
    peer_count: usize,
    peer_last_refreshed: tokio::time::Instant,
}

/// Number of seconds before an idle connection is closed.
const IDLE_CONNECTION_TIMEOUT_SECS: u64 = 60;

/// Number of seconds between refreshing the Kademlia DHT.
const PEER_REFRESH_INTERVAL_SECS: u64 = 4;

/// Static bootstrap nodes for the Kademlia DHT bootstrap step.
const STATIC_BOOTSTRAP_NODES: [&str; 1] =
    ["/ip4/18.156.200.161/tcp/4001/p2p/16Uiu2HAkxwSrnDLZMjhm6JpdTv7h36eNh84Abao3aXVxytQDDrm4"];

/// Static relay nodes for the `P2pCircuit`.
const STATIC_RELAY_NODES: [&str; 1] =
    ["/ip4/3.74.233.98/tcp/4001/p2p/16Uiu2HAm75ZYbLS2h7xRxZgKXxKZrWSGxz2nAL5ESo7GbQYDkWWA"];

impl P2PClient {
    /// Creates a new P2P client with the given keypair and listen address.
    pub fn new(
        keypair: Keypair,
        listen_addr: Multiaddr,
        cancellation: CancellationToken,
    ) -> Result<Self, String> {
        // this is our peerId
        let node_peerid = keypair.public().to_peer_id();
        log::warn!("Compute node peer address: {}", node_peerid);

        // optional static nodes from environment variables
        let (opt_bootstrap_nodes, opt_relay_nodes) = parse_static_nodes_from_env();

        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().port_reuse(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| e.to_string())?
            .with_quic()
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .map_err(|e| e.to_string())?
            .with_behaviour(|key, relay_behavior| Ok(DriaBehaviour::new(key, relay_behavior)))
            .map_err(|e| e.to_string())?
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
        let mut bootstrap_nodes = Vec::from(STATIC_BOOTSTRAP_NODES.map(|s| s.to_string()));
        bootstrap_nodes.extend(opt_bootstrap_nodes.iter().cloned());
        log::info!("Initiating bootstrap.");
        log::debug!("Bootstrap nodes: {:#?}", bootstrap_nodes);
        for addr in bootstrap_nodes {
            if let Ok(addr) = addr.parse::<Multiaddr>() {
                if let Some(peer_id) = addr.iter().find_map(|p| match p {
                    Protocol::P2p(peer_id) => Some(peer_id),
                    _ => None,
                }) {
                    log::info!("Dialling peer: {}", addr);
                    swarm.dial(addr.clone()).map_err(|e| e.to_string())?;
                    log::info!("Adding address to Kademlia routing table");
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                } else {
                    log::warn!("Missing peerID in address: {}", addr);
                }
            } else {
                log::error!("Failed to parse address: {}", addr);
            }
        }

        // do a random-walk on the DHT with a random peer
        log::info!("Searching for random peers.");
        let random_peer = PeerId::random();
        swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(random_peer);
        swarm
            .behaviour_mut()
            .kademlia
            .bootstrap()
            .map_err(|e| e.to_string())?;

        // listen on all interfaces for incoming connections
        log::info!("Listening p2p network on: {}", listen_addr);
        swarm.listen_on(listen_addr).map_err(|e| e.to_string())?;

        let mut relay_nodes = Vec::from(STATIC_RELAY_NODES.map(|s| s.to_string()));
        relay_nodes.extend(opt_relay_nodes.iter().cloned());
        log::info!("Listening to relay nodes.");
        log::debug!("Relay nodes: {:#?}", relay_nodes);
        for addr in relay_nodes {
            if let Ok(addr) = addr.parse::<Multiaddr>() {
                swarm
                    .listen_on(addr.with(Protocol::P2pCircuit))
                    .map_err(|e| e.to_string())?;
            } else {
                // this is not expected happen for static addresses
                log::error!("Failed to parse address: {}", addr);
            }
        }

        Ok(Self {
            swarm,
            cancellation,
            peer_count: 0,
            peer_last_refreshed: tokio::time::Instant::now(),
        })
    }

    /// Subscribe to a topic.
    pub fn subscribe(&mut self, topic_name: &str) -> Result<bool, SubscriptionError> {
        log::debug!("Subscribing to {}", topic_name);

        let topic = gossipsub::IdentTopic::new(topic_name);
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)
    }

    /// Unsubscribe from a topic.
    pub fn unsubscribe(&mut self, topic_name: &str) -> Result<bool, PublishError> {
        log::debug!("Unsubscribing from {}", topic_name);

        let topic = gossipsub::IdentTopic::new(topic_name);
        self.swarm.behaviour_mut().gossipsub.unsubscribe(&topic)
    }

    /// Publish a message to a topic.
    pub fn publish(
        &mut self,
        topic_name: &str,
        message_bytes: Vec<u8>,
    ) -> Result<MessageId, PublishError> {
        log::debug!("Publishing message to topic: {}", topic_name);

        let topic = gossipsub::IdentTopic::new(topic_name);
        let message_id = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, message_bytes)?;

        log::debug!("Published message with ID: {:?}", message_id);
        Ok(message_id)
    }

    /// Returns the list of connected peers within Gossipsub, with a list of subscribed topic hashes by each peer.
    pub fn peers(&self) -> Vec<(&PeerId, Vec<&TopicHash>)> {
        self.swarm
            .behaviour()
            .gossipsub
            .all_peers()
            .collect::<Vec<_>>()
    }

    /// Listens to the Swarm for incoming messages.
    /// This method should be called in a loop to keep the client running.
    /// When a message is received, it will be returned.
    pub async fn process_events(&mut self) -> Option<(PeerId, MessageId, Message)> {
        loop {
            // do a random walk if it has been sometime since we last refreshed it
            if self.peer_last_refreshed.elapsed() > Duration::from_secs(PEER_REFRESH_INTERVAL_SECS)
            {
                let random_peer = PeerId::random();
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .get_closest_peers(random_peer);
                self.peer_last_refreshed = tokio::time::Instant::now();

                // print number of peers
                let latest_peers = self
                    .swarm
                    .behaviour()
                    .gossipsub
                    .all_peers()
                    .collect::<Vec<_>>();
                if latest_peers.len() != self.peer_count {
                    self.peer_count = latest_peers.len();
                    log::info!("Peer Count: {}", latest_peers.len());
                    log::debug!(
                        "Peers: {:#?}",
                        latest_peers
                            .into_iter()
                            .map(|(p, _)| p.to_string())
                            .collect::<Vec<_>>()
                    );
                }
            }

            // wait for next event
            tokio::select! {
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(DriaBehaviourEvent::Kademlia(
                        kad::Event::OutboundQueryProgressed {
                            result: QueryResult::GetClosestPeers(result),
                            ..
                        },
                    )) => self.handle_closest_peers_result(result),
                    SwarmEvent::Behaviour(DriaBehaviourEvent::Identify(identify::Event::Received {
                        peer_id,
                        info,
                    })) => self.handle_identify_event(peer_id, info),
                    SwarmEvent::Behaviour(DriaBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id,
                        message,
                    })) => {
                        return Some((peer_id, message_id, message));
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        log::info!("Local node is listening on {}", address);
                    }
                    _ => log::trace!("Unhandled Swarm Event: {:?}", event),
                },
                _ = self.cancellation.cancelled() => {
                    return None;
                }
            }
        }
    }

    /// Handles identify events to add peer addresses to Kademlia, if protocols match.
    fn handle_identify_event(&mut self, peer_id: PeerId, info: identify::Info) {
        let protocol_match = info.protocols.iter().any(|p| *p == DRIA_PROTO_NAME);
        for addr in info.listen_addrs {
            if protocol_match {
                // if it matches our protocol, add it to the Kademlia routing table
                log::info!("Identify: Received address {}. PeerID is {}", addr, peer_id);

                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, addr);
            } else {
                log::trace!(
                    "Identify: Incoming from different protocol, address {}. PeerID is {}",
                    addr,
                    peer_id
                );
            }
        }
    }

    /// Handles the results of a Kademlia closest peers search, either adding peers to Gossipsub or logging timeout errors.
    fn handle_closest_peers_result(
        &mut self,
        result: Result<GetClosestPeersOk, GetClosestPeersError>,
    ) {
        match result {
            Ok(GetClosestPeersOk { peers, .. }) => {
                if !peers.is_empty() {
                    log::debug!(
                        "Kademlia: Query finished with {} closest peers.",
                        peers.len()
                    );
                    for peer in peers {
                        log::debug!("Gossipsub: Adding peer {peer}");
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .add_explicit_peer(&peer);
                    }
                } else {
                    log::warn!("Kademlia: Query finished with no closest peers.");
                }
            }
            Err(GetClosestPeersError::Timeout { peers, .. }) => {
                if !peers.is_empty() {
                    log::debug!(
                        "Kademlia: Query timed out with {} closest peers.",
                        peers.len()
                    );
                    for peer in peers {
                        log::info!("Gossipsub: Adding peer {peer}");
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .add_explicit_peer(&peer);
                    }
                } else {
                    log::warn!("Kademlia: Query timed out with no closest peers.");
                }
            }
        }
    }
}

/// Parses static bootstrap & relay nodes from environment variables.
/// Returns a tuple of (bootstrap_nodes, relay_nodes).
///
/// The environment variables are:
/// - `DRIA_BOOTSTRAP_NODES`: comma-separated list of bootstrap nodes
/// - `DRIA_RELAY_NODES`: comma-separated list of relay nodes
fn parse_static_nodes_from_env() -> (Vec<String>, Vec<String>) {
    // parse bootstrap nodes
    let bootstrap_nodes = split_comma_separated(std::env::var("DKN_BOOTSTRAP_NODES").ok());
    if bootstrap_nodes.is_empty() {
        log::debug!("No additional bootstrap nodes provided.");
    } else {
        log::debug!("Using additional bootstrap nodes: {:#?}", bootstrap_nodes);
    }

    // parse relay nodes
    let relay_nodes = split_comma_separated(std::env::var("DKN_RELAY_NODES").ok());
    if relay_nodes.is_empty() {
        log::debug!("No additional relay nodes provided.");
    } else {
        log::debug!("Using additional relay nodes: {:#?}", relay_nodes);
    }

    (bootstrap_nodes, relay_nodes)
}
