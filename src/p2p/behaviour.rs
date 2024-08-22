use std::collections::hash_map;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use libp2p::identity::{Keypair, PublicKey};
use libp2p::kad::store::MemoryStore;
use libp2p::{autonat, dcutr, gossipsub, identify, kad, relay, swarm::NetworkBehaviour, PeerId};

use crate::p2p::DRIA_PROTO_NAME;

#[derive(NetworkBehaviour)]
pub struct DriaBehaviour {
    pub(crate) relay: relay::client::Behaviour,
    pub(crate) gossipsub: gossipsub::Behaviour,
    pub(crate) kademlia: kad::Behaviour<MemoryStore>,
    pub(crate) identify: identify::Behaviour,
    pub(crate) autonat: autonat::Behaviour,
    pub(crate) dcutr: dcutr::Behaviour,
}

impl DriaBehaviour {
    pub fn new(key: &Keypair, relay_behavior: relay::client::Behaviour) -> Self {
        let public_key = key.public();
        let peer_id = public_key.to_peer_id();
        Self {
            relay: relay_behavior,
            gossipsub: create_gossipsub_behavior(key.clone()),
            kademlia: create_kademlia_behavior(peer_id),
            autonat: create_autonat_behavior(peer_id),
            identify: create_identify_behavior(public_key),
            dcutr: create_dcutr_behavior(peer_id),
        }
    }
}

/// Configures the Kademlia DHT behavior for the node.
#[inline]
fn create_kademlia_behavior(local_peer_id: PeerId) -> kad::Behaviour<MemoryStore> {
    use kad::{Behaviour, Caching, Config};

    const QUERY_TIMEOUT_SECS: u64 = 5 * 60;
    const RECORD_TTL_SECS: u64 = 30;

    let mut cfg = Config::new(DRIA_PROTO_NAME);
    cfg.set_query_timeout(Duration::from_secs(QUERY_TIMEOUT_SECS))
        .set_record_ttl(Some(Duration::from_secs(RECORD_TTL_SECS)))
        .set_replication_interval(None)
        .set_caching(Caching::Disabled)
        .set_publication_interval(None);

    Behaviour::with_config(local_peer_id, MemoryStore::new(local_peer_id), cfg)
}

/// Configures the Identify behavior to allow nodes to exchange information like supported protocols.
#[inline]
fn create_identify_behavior(local_public_key: PublicKey) -> identify::Behaviour {
    use identify::{Behaviour, Config};

    // TODO: can we use a different protocol version, e.g. `dria/CRATE_VERSION`
    let cfg = Config::new(DRIA_PROTO_NAME.to_string(), local_public_key);

    Behaviour::new(cfg)
}

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
fn create_gossipsub_behavior(id_keys: Keypair) -> gossipsub::Behaviour {
    use gossipsub::{Behaviour, ConfigBuilder, Message, MessageAuthenticity, MessageId};

    /// Message TTL in seconds
    const MESSAGE_TTL_SECS: u64 = 100;

    /// Gossip cache TTL in seconds
    const GOSSIP_TTL_SECS: u64 = 100;

    /// Message capacity for the gossipsub cache
    const MESSAGE_CAPACITY: usize = 100;

    /// Max transmit size for payloads 256 KB
    const MAX_TRANSMIT_SIZE: usize = 262144;

    /// Max IHAVE length, this is much lower than the default
    /// because we don't need historic messages at all
    const MAX_IHAVE_LENGTH: usize = 100;

    // message id's are simply hashes of the message data
    let message_id_fn = |message: &Message| {
        let mut hasher = hash_map::DefaultHasher::new();
        message.data.hash(&mut hasher);
        MessageId::from(hasher.finish().to_string())
    };

    Behaviour::new(
        MessageAuthenticity::Signed(id_keys),
        ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .max_transmit_size(MAX_TRANSMIT_SIZE) // 256 KB
            .message_id_fn(message_id_fn)
            .message_ttl(Duration::from_secs(MESSAGE_TTL_SECS))
            .gossip_ttl(Duration::from_secs(GOSSIP_TTL_SECS))
            .message_capacity(MESSAGE_CAPACITY)
            .max_ihave_length(MAX_IHAVE_LENGTH)
            .build()
            .expect("Valid config"), // TODO: better error handling
    )
    .expect("Valid behaviour") // TODO: better error handling
}
