pub mod constants;
pub mod defaults;

use ecies::PublicKey;
use hex;
use libsecp256k1::SecretKey;
use std::env;
use tokio::time::Duration;

use crate::utils::{crypto::to_address, message::create_content_topic};

use self::{constants::WAKU_HEARTBEAT_TOPIC, defaults::*};

#[allow(non_snake_case)]
#[derive(Debug, Clone)]
pub struct DriaComputeNodeConfig {
    /// Waku container URL
    pub DKN_WAKU_URL: String,
    /// Wallet Private Key
    pub DKN_WALLET_PRIVKEY: SecretKey,
    /// Wallet Public Key
    pub DKN_WALLET_PUBKEY: PublicKey,
    /// Wallet Public Key
    pub DKN_WALLET_ADDRESS: [u8; 20],
    /// Ollama container host
    pub DKN_OLLAMA_HOST: String,
    /// Ollama container port
    pub DKN_OLLAMA_PORT: u16,
}

impl Default for DriaComputeNodeConfig {
    /// Alias for `new`.
    fn default() -> Self {
        Self::new()
    }
}

impl DriaComputeNodeConfig {
    pub fn new() -> Self {
        let secret_key = SecretKey::parse_slice(
            hex::decode(
                env::var("DKN_WALLET_PRIVKEY").unwrap_or(DEFAULT_DKN_WALLET_PRIVKEY.to_string()),
            )
            .unwrap()
            .as_slice(),
        )
        .expect("Could not parse key.");

        let public_key = PublicKey::from_secret_key(&secret_key);

        let address = to_address(&public_key);

        Self {
            DKN_WAKU_URL: env::var("DKN_WAKU_URL").unwrap_or(DEFAULT_DKN_WAKU_URL.to_string()),

            DKN_WALLET_PRIVKEY: secret_key,

            DKN_WALLET_PUBKEY: public_key,

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
