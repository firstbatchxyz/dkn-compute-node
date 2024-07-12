// TODO:
use libp2p::{
    autonat, dcutr, gossipsub, identify, kad, multiaddr::Protocol, noise, relay,
    swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
};
use libp2p::{Multiaddr, PeerId, Swarm, SwarmBuilder};
use libp2p_identity::{Keypair, PublicKey};
use std::error::Error;
use std::time::Duration;

use super::behaviour::DriaBehaviour;

pub struct P2PClient {
    swarm: Swarm<DriaBehaviour>,
}

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

        log::info!("Searching for closest peers.");
        let random_peer: PeerId = Keypair::generate_secp256k1().public().into();
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
        let p2p_addr = "/ip4/0.0.0.0/tcp/4001".parse().unwrap();
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
    pub fn subscribe(&mut self, topic_name: &str) {
        log::debug!("Subscribing to {}", topic_name);

        let topic = gossipsub::IdentTopic::new(topic_name);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .expect("todo handle error");
    }

    /// Unsubscribe from a topic.
    pub fn unsubscribe(&mut self, topic_name: &str) {
        log::debug!("Unsubscribing from {}", topic_name);

        let topic = gossipsub::IdentTopic::new(topic_name);
        self.swarm
            .behaviour_mut()
            .gossipsub
            .unsubscribe(&topic)
            .expect("todo handle error");
    }
}
