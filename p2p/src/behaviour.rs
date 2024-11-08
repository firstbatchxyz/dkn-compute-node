use std::collections::hash_map;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use eyre::{eyre, Context, Result};
use libp2p::identity::{Keypair, PeerId, PublicKey};
use libp2p::kad::store::MemoryStore;
use libp2p::StreamProtocol;
use libp2p::{autonat, connection_limits, dcutr, gossipsub, identify, kad, relay};

#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DriaBehaviour {
    pub relay: relay::client::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub autonat: autonat::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub connection_limits: connection_limits::Behaviour,
}

impl DriaBehaviour {
    pub fn new(
        key: &Keypair,
        relay_behaviour: relay::client::Behaviour,
        identity_protocol: String,
        kademlia_protocol: StreamProtocol,
    ) -> Result<Self> {
        let public_key = key.public();
        let peer_id = public_key.to_peer_id();

        Ok(Self {
            relay: relay_behaviour,
            gossipsub: create_gossipsub_behaviour(peer_id)
                .wrap_err("could not create Gossipsub behaviour")?,
            kademlia: create_kademlia_behaviour(peer_id, kademlia_protocol),
            autonat: create_autonat_behaviour(peer_id),
            dcutr: create_dcutr_behaviour(peer_id),
            identify: create_identify_behaviour(public_key, identity_protocol),
            connection_limits: create_connection_limits_behaviour(),
        })
    }
}

/// Configures the connection limits.
#[inline]
fn create_connection_limits_behaviour() -> connection_limits::Behaviour {
    use connection_limits::{Behaviour, ConnectionLimits};

    /// Number of established outgoing connections limit, this is directly correlated to peer count
    /// so limiting this will cause a limitation on peers as well.
    const EST_OUTGOING_LIMIT: u32 = 450;

    let limits =
        ConnectionLimits::default().with_max_established_outgoing(Some(EST_OUTGOING_LIMIT));

    Behaviour::new(limits)
}

/// Configures the Kademlia DHT behavior for the node.
#[inline]
fn create_kademlia_behaviour(
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
fn create_identify_behaviour(
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
fn create_dcutr_behaviour(local_peer_id: PeerId) -> dcutr::Behaviour {
    use dcutr::Behaviour;

    Behaviour::new(local_peer_id)
}

/// Configures the Autonat behavior to assist in network address translation detection.
#[inline]
fn create_autonat_behaviour(local_peer_id: PeerId) -> autonat::Behaviour {
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
fn create_gossipsub_behaviour(author: PeerId) -> Result<gossipsub::Behaviour> {
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
    Behaviour::new(
        MessageAuthenticity::Author(author),
        ConfigBuilder::default()
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
            .wrap_err(eyre!("could not create Gossipsub config"))?,
    )
    .map_err(|e| eyre!(e))
}
