pub mod defaults;

use self::defaults::*;
use crate::utils::crypto::to_address;
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
        let secret_key = SecretKey::parse_slice(
            hex::decode(env::var("DKN_WALLET_PRIVKEY").unwrap_or_default())
                .unwrap_or_default()
                .as_slice(),
        )
        .unwrap_or(
            // TODO: maybe give error & ask for key specifically?
            SecretKey::parse(DEFAULT_DKN_WALLET_SECRET_KEY)
                .expect("Should decrypt default secret key."),
        );

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

        Self {
            DKN_ADMIN_PUBLIC_KEY: admin_public_key,
            DKN_WALLET_SECRET_KEY: secret_key,
            DKN_WALLET_PUBLIC_KEY: public_key,
            DKN_WALLET_ADDRESS: address,
        }
    }
}

impl Default for DriaComputeNodeConfig {
    /// Alias for `new`.
    fn default() -> Self {
        Self::new()
    }
}
