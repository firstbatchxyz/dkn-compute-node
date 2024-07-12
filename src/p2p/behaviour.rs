use std::collections::hash_map;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use libp2p::identity::{Keypair, PublicKey};
use libp2p::kad::store::MemoryStore;
use libp2p::{
    autonat, dcutr, gossipsub, identify, kad, relay, swarm::NetworkBehaviour, PeerId,
    StreamProtocol,
};

const DRIA_PROTO_NAME: StreamProtocol = StreamProtocol::new("/dria/kad/1.0.0");

#[derive(NetworkBehaviour)]
pub struct DriaBehaviour {
    pub(crate) relay: relay::client::Behaviour,
    pub(crate) gossipsub: gossipsub::Behaviour,
    pub(crate) kademlia: kad::Behaviour<MemoryStore>,
    pub(crate) identify: identify::Behaviour,
    pub(crate) autonat: autonat::Behaviour,
    pub(crate) dcutr: dcutr::Behaviour,
}

pub use DriaBehaviourEvent as DriaEvent;

impl DriaBehaviour {
    pub fn new(key: &Keypair, relay_behavior: relay::client::Behaviour) -> Self {
        let public_key = key.public();
        let peer_id = public_key.to_peer_id();
        Self {
            relay: relay_behavior,
            gossipsub: create_gossipsub_behavior(key.clone()),
            kademlia: create_kademlia_behavior(peer_id),
            identify: create_identify_behavior(public_key.clone()),
            autonat: create_autonat_behavior(public_key),
            dcutr: create_dcutr_behavior(peer_id),
        }
    }
}

/// Configures the Kademlia DHT behavior for the node.
#[inline]
fn create_kademlia_behavior(local_peer_id: PeerId) -> kad::Behaviour<MemoryStore> {
    use kad::{Behaviour, Config};

    let mut cfg = Config::default();
    cfg.set_protocol_names(vec![DRIA_PROTO_NAME])
        .set_query_timeout(Duration::from_secs(5 * 60));

    Behaviour::with_config(local_peer_id, MemoryStore::new(local_peer_id), cfg)
}

/// Configures the Identify behavior to allow nodes to exchange information like supported protocols.
#[inline]
fn create_identify_behavior(local_public_key: PublicKey) -> identify::Behaviour {
    use identify::{Behaviour, Config};

    Behaviour::new(Config::new(DRIA_PROTO_NAME.to_string(), local_public_key))
}

#[inline]
fn create_dcutr_behavior(local_peer_id: PeerId) -> dcutr::Behaviour {
    use dcutr::Behaviour;

    Behaviour::new(local_peer_id)
}

/// Configures the Autonat behavior to assist in network address translation detection.
#[inline]
fn create_autonat_behavior(key: PublicKey) -> autonat::Behaviour {
    use autonat::{Behaviour, Config};

    Behaviour::new(
        key.to_peer_id(),
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
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .expect("Valid config"), // TODO: better error handling
    )
    .expect("Valid behaviour") // TODO: better error handling
}
