pub mod models;
pub mod ollama;

use crate::utils::crypto::to_address;
use ecies::PublicKey;
use libsecp256k1::{PublicKeyFormat, SecretKey};
use models::parse_models_string;
use ollama::OllamaConfig;
use ollama_workflows::{Model, ModelProvider};
use std::env;

#[derive(Debug, Clone)]
pub struct DriaComputeNodeConfig {
    /// Wallet secret/private key.
    pub secret_key: SecretKey,
    /// Wallet public key, derived from the secret key.
    pub public_key: PublicKey,
    /// Wallet address, derived from the public key.
    pub address: [u8; 20],
    /// Admin public key, used for message authenticity.
    pub admin_public_key: PublicKey,
    /// Available models for the node.
    pub models: Vec<(ModelProvider, Model)>,
    /// P2P listen address as a string, e.g. `/ip4/0.0.0.0/tcp/4001`.
    pub p2p_listen_addr: String,
    /// Ollama configuration.
    /// Even if Ollama is not used, we store the host & port her.
    /// If Ollama is used, this config will be respected.
    pub ollama: OllamaConfig,
}

/// 32 byte secret key hex(b"node") * 8, dummy only
pub(crate) const DEFAULT_DKN_WALLET_SECRET_KEY: &[u8; 32] =
    &hex_literal::hex!("6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465");

/// 33 byte compressed public key of secret key from hex(b"dria) * 8, dummy only
pub(crate) const DEFAULT_DKN_ADMIN_PUBLIC_KEY: &[u8; 33] =
    &hex_literal::hex!("0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658");

/// The default P2P network listen address.
pub(crate) const DEFAULT_P2P_LISTEN_ADDR: &str = "/ip4/0.0.0.0/tcp/4001";

impl DriaComputeNodeConfig {
    pub fn new() -> Self {
        let secret_key = match env::var("DKN_WALLET_SECRET_KEY") {
            Ok(secret_env) => {
                let secret_dec =
                    hex::decode(secret_env).expect("Secret key should be 32-bytes hex encoded.");
                SecretKey::parse_slice(&secret_dec).expect("Secret key should be parseable.")
            }
            Err(_) => SecretKey::parse(DEFAULT_DKN_WALLET_SECRET_KEY)
                .expect("Should decrypt default secret key."),
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

        let admin_public_key = PublicKey::parse_slice(
            hex::decode(env::var("DKN_ADMIN_PUBLIC_KEY").unwrap_or_default())
                .unwrap_or_default()
                .as_slice(),
            Some(PublicKeyFormat::Compressed),
        )
        .unwrap_or(
            PublicKey::parse_compressed(DEFAULT_DKN_ADMIN_PUBLIC_KEY)
                .expect("Should decrypt default Admin public key."),
        );
        log::info!(
            "Admin Public Key: 0x{}",
            hex::encode(admin_public_key.serialize_compressed())
        );

        let address = to_address(&public_key);
        log::info!("Node Address:     0x{}", hex::encode(address));

        let models = parse_models_string(env::var("DKN_MODELS").ok());
        log::info!(
            "Models: {}",
            serde_json::to_string(&models).unwrap_or_default()
        );

        let p2p_listen_addr =
            env::var("DKN_P2P_LISTEN_ADDR").unwrap_or(DEFAULT_P2P_LISTEN_ADDR.to_string());

        let ollama = OllamaConfig::new();

        Self {
            admin_public_key,
            secret_key,
            public_key,
            address,
            models,
            p2p_listen_addr,
            ollama,
        }
    }
}

impl Default for DriaComputeNodeConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        env::set_var(
            "DKN_WALLET_SECRET_KEY",
            "6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465",
        );
        let cfg = DriaComputeNodeConfig::new();
        assert_eq!(
            hex::encode(cfg.address),
            "1f56f6131705fbf19371122c80d7a2d40fcf9a68"
        );
    }
}
