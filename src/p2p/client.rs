use libp2p::futures::StreamExt;
use libp2p::gossipsub::{Message, MessageId, PublishError, SubscriptionError};
use libp2p::kad::{GetClosestPeersError, GetClosestPeersOk, QueryResult};
// TODO:
use libp2p::{gossipsub, identify, kad, multiaddr::Protocol, noise, swarm::SwarmEvent, tcp, yamux};
use libp2p::{Multiaddr, PeerId, Swarm, SwarmBuilder};
use libp2p_identity::Keypair;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::p2p::behaviour::DriaBehaviourEvent;

use super::behaviour::DriaBehaviour;
use super::DRIA_PROTO_NAME;

pub struct P2PClient {
    swarm: Swarm<DriaBehaviour>,
}

// FIXME: lots of map_err to strings, should be handled better

impl P2PClient {
    // TODO: change error type
    pub fn new(local_key: Keypair) -> Result<Self, String> {
        let local_peer_id = local_key.public().to_peer_id();

        let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
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
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        log::info!("My peer addr is {:?}", local_peer_id);

        // FIXME: can this be done in behaviour settings?
        swarm
            .behaviour_mut()
            .kademlia
            .set_mode(Some(libp2p::kad::Mode::Server)); // TODO: is this safe?

        log::info!("Initiating bootstrap.");
        let bootstrap_nodes = vec![
                "/ip4/18.156.200.161/tcp/4001/p2p/16Uiu2HAkxwSrnDLZMjhm6JpdTv7h36eNh84Abao3aXVxytQDDrm4",
            ];
        for addr in bootstrap_nodes {
            if let Ok(addr) = addr.parse::<Multiaddr>() {
                if let Some(peer_id) = addr.iter().find_map(|p| match p {
                    Protocol::P2p(peer_id) => Some(peer_id),
                    _ => None,
                }) {
                    log::info!("Dialling peer {}.", addr);
                    swarm.dial(addr.clone()).map_err(|e| e.to_string())?;
                    log::info!("Adding address to Kademlia routing table.");
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                } else {
                    log::warn!("Missing peer ID in address: {}", addr);
                }
            } else {
                // TODO: this should not happen for static addresses
                log::warn!("Failed to parse address: {}", addr);
            }
        }

        log::info!("Searching for random peers.");
        // let random_peer: PeerId = Keypair::generate_secp256k1().public().into(); // FIXME: do as below?
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

        // Listen on all interfaces for incoming connections
        // FIXME: configure this address
        let p2p_addr = "/ip4/0.0.0.0/tcp/4001"
            .parse()
            .expect("Provided p2p address should be parsable");
        log::info!("Listening p2p network on: {}", p2p_addr);
        swarm.listen_on(p2p_addr).map_err(|e| e.to_string())?;

        log::info!("Listening to relays nodes.");
        let relay_nodes = vec![
            "/ip4/3.74.233.98/tcp/4001/p2p/16Uiu2HAm75ZYbLS2h7xRxZgKXxKZrWSGxz2nAL5ESo7GbQYDkWWA",
        ];
        for addr in relay_nodes {
            if let Ok(addr) = addr.parse::<Multiaddr>() {
                swarm
                    .listen_on(addr.with(Protocol::P2pCircuit))
                    .map_err(|e| e.to_string())?;
            } else {
                // TODO: this should not happen for static addresses
                log::warn!("Failed to parse address: {}", addr);
            }
        }

        Ok(Self { swarm })
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
        log::debug!("Publishing to {}", topic_name);

        let topic = gossipsub::IdentTopic::new(topic_name);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, message_bytes)
    }

    /// Listens to the Swarm for incoming messages.
    /// This method should be called in a loop to keep the client running.
    /// When a message is received, it will be returned.
    pub async fn process_events(
        &mut self,
        cancellation: CancellationToken,
    ) -> Option<(PeerId, MessageId, Message)> {
        loop {
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
                        log::info!("Local node is listening on {address}");
                    }
                    _ => log::debug!("Unhandled swarm Event {:?}", event),
                },
                _ = cancellation.cancelled() => {
                    return None;
                }
            }
        }
    }

    /// Handles identify events to add peer addresses to Kademlia, if protocols match.
    fn handle_identify_event(&mut self, peer_id: PeerId, info: identify::Info) {
        let protocol_match = info.protocols.iter().any(|p| *p == DRIA_PROTO_NAME);
        for addr in info.listen_addrs {
            let prefix = if protocol_match {
                "received"
            } else {
                "Incoming from different protocol"
            };
            log::info!(
                "{prefix} addr {addr} through identify. Peer is {:?}",
                peer_id
            );
            if protocol_match {
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, addr);
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
                    log::info!("Query finished with closest peers: {:#?}", peers);
                    for peer in peers {
                        log::info!("gossipsub adding peer {peer}");
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .add_explicit_peer(&peer);
                    }
                } else {
                    log::info!("Query finished with no closest peers.");
                }
            }
            Err(GetClosestPeersError::Timeout { peers, .. }) => {
                if !peers.is_empty() {
                    log::warn!("Query timed out with closest peers: {:#?}", peers);
                    for peer in peers {
                        log::info!("gossipsub adding peer {peer}");
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .add_explicit_peer(&peer);
                    }
                } else {
                    log::warn!("Query timed out with no closest peers.");
                }
            }
        }
    }

    // TODO: add method to get peers (specifically their count) from Kademlia
}
