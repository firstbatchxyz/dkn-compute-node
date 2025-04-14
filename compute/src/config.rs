use dkn_p2p::{
    libp2p::{Multiaddr, PeerId},
    DriaNetworkType,
};
use dkn_workflows::DriaWorkflowsConfig;
use eyre::{eyre, Result};
use libsecp256k1::{PublicKey, SecretKey};
use std::{env, str::FromStr};

use dkn_utils::{
    crypto::{public_key_to_address, secret_to_keypair},
    SemanticVersion,
};

const DEFAULT_TASK_BATCH_SIZE: usize = 5;
const DEFAULT_P2P_LISTEN_ADDR: &str = "/ip4/0.0.0.0/tcp/4001";

#[derive(Debug, Clone)]
pub struct DriaComputeNodeConfig {
    /// Wallet secret/private key.
    pub secret_key: SecretKey,
    /// Wallet public key, derived from the secret key.
    pub public_key: PublicKey,
    /// Wallet address in hex without `0x` prefix, derived from the public key.
    pub address: String,
    /// Peer ID of the node.
    pub peer_id: PeerId,
    /// Compute node version.
    pub version: SemanticVersion,
    /// P2P listen address, e.g. `/ip4/0.0.0.0/tcp/4001`.
    pub p2p_listen_addr: Multiaddr,
    /// Workflow configurations, e.g. models and providers.
    pub workflows: DriaWorkflowsConfig,
    /// Network type of the node.
    pub network_type: DriaNetworkType,
    /// Batch size for batchable tasks (e.g. API-based ones).
    ///
    /// A higher value will help execute more tasks concurrently,
    /// at the risk of hitting rate-limits.
    pub batch_size: usize,
}

#[allow(clippy::new_without_default)]
impl DriaComputeNodeConfig {
    /// Creates new config from environment variables.
    pub fn new(workflows: DriaWorkflowsConfig) -> Self {
        let secret_key = match env::var("DKN_WALLET_SECRET_KEY") {
            Ok(secret_env) => {
                let secret_dec = hex::decode(secret_env.trim_start_matches("0x"))
                    .expect("Secret key should be 32-bytes hex encoded.");

                // if secret key is all-zeros, create one randomly
                // this is useful for testing & creating nodes on the fly
                if secret_dec.iter().all(|b| b == &0) {
                    SecretKey::random(&mut rand::thread_rng())
                } else {
                    SecretKey::parse_slice(&secret_dec).expect("Secret key should be parseable.")
                }
            }
            Err(err) => {
                log::error!("No secret key provided: {}", err);
                panic!("Please provide a secret key.");
            }
        };
        log::info!(
            "Node Secret Key:  0x{}{}",
            hex::encode(&secret_key.serialize()[0..1]),
            ".".repeat(64)
        );

        let public_key = PublicKey::from_secret_key(&secret_key);
        log::info!(
            "Node Public Key:  0x{}",
            hex::encode(public_key.serialize_compressed())
        );

        // print address
        let address = hex::encode(public_key_to_address(&public_key));
        log::info!("Node Address:     0x{}", address);

        // to this here to log the peer id at start
        let peer_id = secret_to_keypair(&secret_key).public().to_peer_id();
        log::info!("Node PeerID:      {}", peer_id);

        // parse listen address
        let p2p_listen_addr_str = env::var("DKN_P2P_LISTEN_ADDR")
            .map(|addr| addr.trim_matches('"').to_string())
            .unwrap_or(DEFAULT_P2P_LISTEN_ADDR.to_string());
        let p2p_listen_addr = Multiaddr::from_str(&p2p_listen_addr_str)
            .expect("could not parse the given P2P listen address.");

        // parse network type
        let network_type = env::var("DKN_NETWORK")
            .map(|s| DriaNetworkType::from(s.as_str()))
            .unwrap_or_default();

        // parse batch size
        let batch_size = env::var("DKN_BATCH_SIZE")
            .map(|s| s.parse::<usize>().unwrap_or(DEFAULT_TASK_BATCH_SIZE))
            .unwrap_or(DEFAULT_TASK_BATCH_SIZE);

        // parse version
        let version = env!("CARGO_PKG_VERSION")
            .parse()
            .expect("could not parse version");

        Self {
            secret_key,
            public_key,
            address,
            peer_id,
            version,
            workflows,
            p2p_listen_addr,
            network_type,
            batch_size,
        }
    }

    /// Asserts that the configured listen address is free.
    /// Throws an error if the address is already in use.
    ///
    /// Uses `is_port_reachable` function internally, which makes a simple
    /// TCP connection to the given address.
    ///
    /// Can be inlined because the function is small and called only once.
    #[inline]
    pub fn assert_address_not_in_use(&self) -> Result<()> {
        use dkn_p2p::libp2p::multiaddr::Protocol;
        use port_check::is_port_reachable;
        use std::net::{Ipv4Addr, SocketAddrV4};

        let address_in_use = self
            .p2p_listen_addr
            .iter()
            // find the port within our multiaddr
            .find_map(|protocol| match protocol {
                Protocol::Tcp(port) => Some(port),
                _ => None,
            })
            // check if its reachable or not
            .map(|port| is_port_reachable(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)))
            .unwrap_or_else(|| {
                log::error!(
                    "could not find any TCP port in the given address: {:?}",
                    self.p2p_listen_addr
                );
                false
            });

        if address_in_use {
            return Err(eyre!(
                "Listen address {} is already in use.",
                self.p2p_listen_addr
            ));
        }

        Ok(())
    }

    /// Checks the network specific configurations.
    pub fn check_network_specific(&self) -> Result<()> {
        // if network is `pro`, we require Jina and Serper to be present.
        if self.network_type == DriaNetworkType::Pro {
            if !self.workflows.jina.has_api_key() {
                return Err(eyre!("Jina is required for the Pro network."));
            }
            if !self.workflows.serper.has_api_key() {
                return Err(eyre!("Serper is required for the Pro network."));
            }
        }

        Ok(())
    }
}
