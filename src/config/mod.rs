pub mod defaults;

use self::defaults::*;
use crate::utils::crypto::to_address;
use ecies::PublicKey;
use libsecp256k1::{PublicKeyFormat, SecretKey};
use std::env;

/// 32 byte secret key hex(b"node") * 8
/// address:
#[cfg(test)]
pub const DEFAULT_DKN_WALLET_SECRET_KEY: &[u8; 32] =
    &hex_literal::hex!("6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465");

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

#[cfg(test)]
fn prepare_secret() -> SecretKey {
    SecretKey::parse(DEFAULT_DKN_WALLET_SECRET_KEY).expect("Should decrypt default secret key.")
}

#[cfg(not(test))]
fn prepare_secret() -> SecretKey {
    let secret_env =
        env::var("DKN_WALLET_SECRET_KEY").expect("Secret key should be provided in .env.");
    let secret_dec = hex::decode(secret_env).expect("Secret key should be 32-bytes hex encoded.");
    SecretKey::parse_slice(&secret_dec).expect("Secret key should be parseable.")
}

impl DriaComputeNodeConfig {
    pub fn new() -> Self {
        let secret_key = prepare_secret();

        let public_key = PublicKey::from_secret_key(&secret_key);

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

        let address = to_address(&public_key);

        log::info!("Address:    0x{}", hex::encode(address));
        log::info!(
            "Node Public Key: 0x{}",
            hex::encode(public_key.serialize_compressed())
        );
        log::info!(
            "Node Secret Key: 0x{}...",
            hex::encode(&secret_key.serialize()[0..1]),
        );
        log::info!(
            "Admin Public Key: 0x{}",
            hex::encode(admin_public_key.serialize_compressed())
        );

        Self {
            DKN_ADMIN_PUBLIC_KEY: admin_public_key,
            DKN_WALLET_SECRET_KEY: secret_key,
            DKN_WALLET_PUBLIC_KEY: public_key,
            DKN_WALLET_ADDRESS: address,
        }
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
            hex::encode(cfg.DKN_WALLET_ADDRESS),
            "1f56f6131705fbf19371122c80d7a2d40fcf9a68"
        );
    }
}
