pub mod constants;
pub mod models;

use crate::utils::crypto::to_address;
use constants::*;
use ecies::PublicKey;
use libsecp256k1::{PublicKeyFormat, SecretKey};
use models::parse_dkn_models;
use ollama_workflows::{Model, ModelProvider};
use std::env;

#[allow(non_snake_case)]
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
}

impl DriaComputeNodeConfig {
    pub fn new() -> Self {
        let secret_key = match env::var(DKN_WALLET_SECRET_KEY) {
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
            hex::decode(env::var(DKN_ADMIN_PUBLIC_KEY).unwrap_or_default())
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

        let models = parse_dkn_models(env::var(DKN_MODELS).unwrap_or_default());
        log::info!(
            "Models: {}",
            serde_json::to_string(&models).unwrap_or_default()
        );

        Self {
            admin_public_key,
            secret_key,
            public_key,
            address,
            models,
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
            DKN_WALLET_SECRET_KEY,
            "6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465",
        );
        let cfg = DriaComputeNodeConfig::new();
        assert_eq!(
            hex::encode(cfg.address),
            "1f56f6131705fbf19371122c80d7a2d40fcf9a68"
        );
    }
}
