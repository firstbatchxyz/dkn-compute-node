pub mod constants;
pub mod tasks;

use crate::utils::crypto::to_address;
use constants::*;
use ecies::PublicKey;
use libsecp256k1::{PublicKeyFormat, SecretKey};
use std::env;

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct DriaComputeNodeConfig {
    /// Wallet secret/private key.
    pub(crate) DKN_WALLET_SECRET_KEY: SecretKey,
    /// Wallet public key, derived from the secret key.
    pub DKN_WALLET_PUBLIC_KEY: PublicKey,
    /// Wallet address, derived from the public key.
    pub DKN_WALLET_ADDRESS: [u8; 20],
    /// Admin public key, used for message authenticity.
    pub DKN_ADMIN_PUBLIC_KEY: PublicKey,
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

        let public_key = PublicKey::from_secret_key(&secret_key);

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

        let address = to_address(&public_key);

        log::info!(
            "Admin Public Key: 0x{}",
            hex::encode(admin_public_key.serialize_compressed())
        );

        log::info!("Node Address:     0x{}", hex::encode(address));
        log::info!(
            "Node Public Key:  0x{}",
            hex::encode(public_key.serialize_compressed())
        );
        log::info!(
            "Node Secret Key:  0x{}{}",
            hex::encode(&secret_key.serialize()[0..1]),
            ".".repeat(64)
        );

        Self {
            DKN_ADMIN_PUBLIC_KEY: admin_public_key,
            DKN_WALLET_SECRET_KEY: secret_key,
            DKN_WALLET_PUBLIC_KEY: public_key,
            DKN_WALLET_ADDRESS: address,
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
            hex::encode(cfg.DKN_WALLET_ADDRESS),
            "1f56f6131705fbf19371122c80d7a2d40fcf9a68"
        );
    }
}
