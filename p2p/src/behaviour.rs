use std::collections::hash_map;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use eyre::{eyre, Result};
use libp2p::identity::{Keypair, PeerId, PublicKey};
use libp2p::kad::store::MemoryStore;
use libp2p::StreamProtocol;
use libp2p::{autonat, dcutr, gossipsub, identify, kad, relay};

#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DriaBehaviour {
    pub(crate) relay: relay::client::Behaviour,
    pub(crate) gossipsub: gossipsub::Behaviour,
    pub(crate) kademlia: kad::Behaviour<MemoryStore>,
    pub(crate) identify: identify::Behaviour,
    pub(crate) autonat: autonat::Behaviour,
    pub(crate) dcutr: dcutr::Behaviour,
}

impl DriaBehaviour {
    pub fn new(
        key: &Keypair,
        relay_behavior: relay::client::Behaviour,
        identity_protocol: String,
        kademlia_protocol: StreamProtocol,
    ) -> Result<Self> {
        let public_key = key.public();
        let peer_id = public_key.to_peer_id();
        Ok(Self {
            relay: relay_behavior,
            gossipsub: create_gossipsub_behavior(peer_id)?,
            kademlia: create_kademlia_behavior(peer_id, kademlia_protocol),
            autonat: create_autonat_behavior(peer_id),
            dcutr: create_dcutr_behavior(peer_id),
            identify: create_identify_behavior(public_key, identity_protocol),
        })
    }
}

/// Configures the Kademlia DHT behavior for the node.
#[inline]
fn create_kademlia_behavior(
    local_peer_id: PeerId,
    protocol_name: StreamProtocol,
) -> kad::Behaviour<MemoryStore> {
    use kad::{Behaviour, Config};

    const QUERY_TIMEOUT_SECS: u64 = 5 * 60;
    const RECORD_TTL_SECS: u64 = 30;

    let mut cfg = Config::new(protocol_name);
    cfg.set_query_timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .set_record_ttl(Some(Duration::from_secs(RECORD_TTL_SECS)));

    Behaviour::with_config(local_peer_id, MemoryStore::new(local_peer_id), cfg)
}

/// Configures the Identify behavior to allow nodes to exchange information like supported protocols.
#[inline]
fn create_identify_behavior(
    local_public_key: PublicKey,
    protocol_version: String,
) -> identify::Behaviour {
    use identify::{Behaviour, Config};

    let cfg = Config::new(protocol_version, local_public_key);

    Behaviour::new(cfg)
}

/// Configures the Dcutr behavior to allow nodes to connect via hole-punching.
/// It uses a Relay for the hole-punching process, and if it succeeds the peers are
/// connected directly without the need for the relay; otherwise, they keep using the relay.
#[inline]
fn create_dcutr_behavior(local_peer_id: PeerId) -> dcutr::Behaviour {
    use dcutr::Behaviour;

    Behaviour::new(local_peer_id)
}

/// Configures the Autonat behavior to assist in network address translation detection.
#[inline]
fn create_autonat_behavior(local_peer_id: PeerId) -> autonat::Behaviour {
    use autonat::{Behaviour, Config};

    Behaviour::new(
        local_peer_id,
        Config {
            only_global_ips: false,
            ..Default::default()
        },
    )
}

/// Configures the Gossipsub behavior for pub/sub messaging across peers.
#[inline]
fn create_gossipsub_behavior(author: PeerId) -> Result<gossipsub::Behaviour> {
    use gossipsub::{
        Behaviour, ConfigBuilder, Message, MessageAuthenticity, MessageId, ValidationMode,
    };

    /// Message TTL in seconds
    const MESSAGE_TTL_SECS: u64 = 100;

    /// We accept permissive validation mode, meaning that we accept all messages
    /// and check their fields based on whether they exist or not.
    const VALIDATION_MODE: ValidationMode = ValidationMode::Permissive;

    /// Heartbeat interval in seconds
    const HEARTBEAT_INTERVAL_SECS: u64 = 10;

    /// Duplicate cache time in seconds
    const DUPLICATE_CACHE_TIME_SECS: u64 = 120;

    /// Gossip cache TTL in seconds
    const GOSSIP_TTL_SECS: u64 = 100;

    /// Message capacity for the gossipsub cache
    const MESSAGE_CAPACITY: usize = 100;

    /// Max transmit size for payloads 256 KB
    const MAX_TRANSMIT_SIZE: usize = 256 << 10;

    /// Max IHAVE length, this is much lower than the default
    /// because we don't need historic messages at all
    const MAX_IHAVE_LENGTH: usize = 100;

    /// Max size of the send queue
    /// This helps to avoid memory exhaustion during high load
    const MAX_SEND_QUEUE_SIZE: usize = 400;

    // message id's are simply hashes of the message data, via SipHash13
    let message_id_fn = |message: &Message| {
        let mut hasher = hash_map::DefaultHasher::new();
        message.data.hash(&mut hasher);
        let digest = hasher.finish();
        MessageId::from(digest.to_be_bytes())
    };

    // TODO: add data transform here later
    let config = match ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS))
        .max_transmit_size(MAX_TRANSMIT_SIZE)
        .message_id_fn(message_id_fn)
        .message_capacity(MESSAGE_CAPACITY)
        .message_ttl(Duration::from_secs(MESSAGE_TTL_SECS))
        .gossip_ttl(Duration::from_secs(GOSSIP_TTL_SECS))
        .duplicate_cache_time(Duration::from_secs(DUPLICATE_CACHE_TIME_SECS))
        .max_ihave_length(MAX_IHAVE_LENGTH)
        .send_queue_size(MAX_SEND_QUEUE_SIZE)
        .validation_mode(VALIDATION_MODE)
        .validate_messages()
        .build()
    {
        Ok(config) => config,
        Err(e) => {
            return Err(eyre!("Failed to create gossipsub config: {}", e));
        }
    };

    match Behaviour::new(MessageAuthenticity::Author(author), config) {
        Ok(behaviour) => Ok(behaviour),
        Err(e) => Err(eyre!("Failed to create gossipsub behaviour: {}", e)),
    }
}
