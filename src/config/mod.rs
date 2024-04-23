pub mod constants;
pub mod defaults;

use self::defaults::*;
use crate::utils::crypto::to_address;
use ecies::PublicKey;
use libsecp256k1::{PublicKeyFormat, SecretKey};
use std::env;

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct DriaComputeNodeConfig {
    /// Waku (nwaku) container URL.
    pub DKN_WAKU_URL: String,
    /// Wallet secret/private key.
    pub DKN_WALLET_SECRET_KEY: SecretKey,
    /// Wallet public key, derived from the secret key.
    pub DKN_WALLET_PUBLIC_KEY: PublicKey,
    /// Wallet address, derived from the public key.
    pub DKN_WALLET_ADDRESS: [u8; 20],
    /// Admin public key, used for message authenticity.
    pub DKN_ADMIN_PUBLIC_KEY: PublicKey,
    /// Ollama container host.
    pub DKN_OLLAMA_HOST: String,
    /// Ollama container port.
    pub DKN_OLLAMA_PORT: u16,
}

impl DriaComputeNodeConfig {
    pub fn new() -> Self {
        let secret_key = SecretKey::parse_slice(
            hex::decode(
                env::var("DKN_WALLET_PRIVKEY").unwrap_or(DEFAULT_DKN_WALLET_SECRET_KEY.to_string()),
            )
            .unwrap()
            .as_slice(),
        )
        .expect("Could not parse secret key.");

        let public_key = PublicKey::from_secret_key(&secret_key);

        let admin_public_key = PublicKey::parse_slice(
            hex::decode(
                env::var("DKN_ADMIN_PUBLIC_KEY")
                    .unwrap_or(DEFAULT_DKN_ADMIN_PUBLIC_KEY.to_string()),
            )
            .unwrap()
            .as_slice(),
            Some(PublicKeyFormat::Compressed),
        )
        .expect("Could not parse public key.");

        let address = to_address(&public_key);

        Self {
            DKN_ADMIN_PUBLIC_KEY: admin_public_key,

            DKN_WAKU_URL: env::var("DKN_WAKU_URL").unwrap_or(DEFAULT_DKN_WAKU_URL.to_string()),

            DKN_WALLET_SECRET_KEY: secret_key,
            DKN_WALLET_PUBLIC_KEY: public_key,
            DKN_WALLET_ADDRESS: address,

            DKN_OLLAMA_HOST: env::var("DKN_OLLAMA_HOST")
                .unwrap_or(DEFAULT_DKN_OLLAMA_HOST.to_string()),
            DKN_OLLAMA_PORT: env::var("DKN_OLLAMA_PORT")
                .unwrap_or(DEFAULT_DKN_OLLAMA_PORT.to_string())
                .parse::<u16>()
                .expect("Could not parse port."),
        }
    }
}

impl Default for DriaComputeNodeConfig {
    /// Alias for `new`.
    fn default() -> Self {
        Self::new()
    }
}
