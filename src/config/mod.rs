mod models;
mod ollama;
mod openai;

use crate::utils::crypto::to_address;
use libsecp256k1::{PublicKey, SecretKey};
use models::ModelConfig;
use ollama::OllamaConfig;
use ollama_workflows::ModelProvider;
use openai::OpenAIConfig;

use std::{env, time::Duration};

/// Timeout duration for checking model performance during a generation.
const CHECK_TIMEOUT_DURATION: Duration = Duration::from_secs(80);

/// Minimum tokens per second (TPS) for checking model performance during a generation.
const CHECK_TPS: f64 = 5.0;

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
    /// P2P listen address as a string, e.g. `/ip4/0.0.0.0/tcp/4001`.
    pub p2p_listen_addr: String,
    /// Available LLM models & providers for the node.
    pub model_config: ModelConfig,
    /// Even if Ollama is not used, we store the host & port here.
    /// If Ollama is used, this config will be respected during its instantiations.
    pub ollama_config: OllamaConfig,
    /// OpenAI API key & its service check implementation.
    pub openai_config: OpenAIConfig,
}

/// The default P2P network listen address.
pub(crate) const DEFAULT_P2P_LISTEN_ADDR: &str = "/ip4/0.0.0.0/tcp/4001";

#[allow(clippy::new_without_default)]
impl DriaComputeNodeConfig {
    /// Creates new config from environment variables.
    pub fn new() -> Self {
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

        let admin_public_key = match env::var("DKN_ADMIN_PUBLIC_KEY") {
            Ok(admin_public_key) => {
                let pubkey_dec = hex::decode(admin_public_key.trim_start_matches("0x"))
                    .expect("Admin public key should be 33-bytes hex encoded.");
                PublicKey::parse_slice(&pubkey_dec, None)
                    .expect("Admin public key should be parseable.")
            }
            Err(err) => {
                log::error!("No admin public key provided: {}", err);
                panic!("Please provide an admin public key.");
            }
        };
        log::info!(
            "Admin Public Key: 0x{}",
            hex::encode(admin_public_key.serialize_compressed())
        );

        let address = to_address(&public_key);
        log::info!("Node Address:     0x{}", hex::encode(address));

        let model_config = ModelConfig::new_from_csv(env::var("DKN_MODELS").ok());
        if model_config.models.is_empty() {
            log::error!("No models were provided, make sure to restart with at least one model provided within DKN_MODELS.");
            panic!("No models provided.");
        }
        log::info!("Models: {}", model_config);

        let p2p_listen_addr = env::var("DKN_P2P_LISTEN_ADDR")
            .map(|addr| addr.trim_matches('"').to_string())
            .unwrap_or(DEFAULT_P2P_LISTEN_ADDR.to_string());

        Self {
            admin_public_key,
            secret_key,
            public_key,
            address,
            model_config,
            p2p_listen_addr,
            ollama_config: OllamaConfig::new(),
            openai_config: OpenAIConfig::new(),
        }
    }

    /// Check if the required compute services are running.
    /// This has several steps:
    ///
    /// - If Ollama models are used, hardcoded models are checked locally, and for
    ///   external models, the workflow is tested with a simple task with timeout.
    /// - If OpenAI models are used, the API key is checked and the models are tested
    ///
    /// If both type of models are used, both services are checked.
    /// In the end, bad models are filtered out and we simply check if we are left if any valid models at all.
    /// If not, an error is returned.
    pub async fn check_services(&mut self) -> Result<(), String> {
        log::info!("Checking configured services.");

        // TODO: can refactor (provider, model) logic here
        let unique_providers = self.model_config.get_providers();

        let mut good_models = Vec::new();

        // if Ollama is a provider, check that it is running & Ollama models are pulled (or pull them)
        if unique_providers.contains(&ModelProvider::Ollama) {
            let ollama_models = self
                .model_config
                .get_models_for_provider(ModelProvider::Ollama);

            // ensure that the models are pulled / pull them if not
            let good_ollama_models = self
                .ollama_config
                .check(ollama_models, CHECK_TIMEOUT_DURATION, CHECK_TPS)
                .await?;
            good_models.extend(
                good_ollama_models
                    .into_iter()
                    .map(|m| (ModelProvider::Ollama, m)),
            );
        }

        // if OpenAI is a provider, check that the API key is set
        if unique_providers.contains(&ModelProvider::OpenAI) {
            let openai_models = self
                .model_config
                .get_models_for_provider(ModelProvider::OpenAI);

            let good_openai_models = self.openai_config.check(openai_models).await?;
            good_models.extend(
                good_openai_models
                    .into_iter()
                    .map(|m| (ModelProvider::OpenAI, m)),
            );
        }

        // update good models
        if good_models.is_empty() {
            Err("No good models found, please check logs for errors.".into())
        } else {
            self.model_config.models = good_models;
            Ok(())
        }
    }
}

#[cfg(test)]
impl Default for DriaComputeNodeConfig {
    /// Creates a new config with dummy values.
    ///
    /// Should only be used for testing purposes.
    fn default() -> Self {
        env::set_var(
            "DKN_ADMIN_PUBLIC_KEY",
            "0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658",
        );
        env::set_var(
            "DKN_WALLET_SECRET_KEY",
            "6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465",
        );
        env::set_var("DKN_MODELS", "gpt-3.5-turbo");

        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_and_model_parsing() {
        let cfg = DriaComputeNodeConfig::default();
        assert_eq!(
            hex::encode(cfg.address),
            // address of the default secret key
            "1f56f6131705fbf19371122c80d7a2d40fcf9a68"
        );

        env::set_var(
            "DKN_ADMIN_PUBLIC_KEY",
            "0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658",
        );
        env::set_var(
            "DKN_WALLET_SECRET_KEY",
            "6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465",
        );
        env::set_var("DKN_MODELS", "phi3:3.8b,gpt-3.5-turbo");
    }
}
