mod models;
mod ollama;

use crate::utils::crypto::to_address;
use libsecp256k1::{PublicKey, SecretKey};
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
    ///
    /// Even if Ollama is not used, we store the host & port here.
    /// If Ollama is used, this config will be respected during its instantiations.
    pub ollama: OllamaConfig,
}

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
                let pubkey_dec = hex::decode(admin_public_key)
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

        let models = parse_models_string(env::var("DKN_MODELS").ok());
        if models.is_empty() {
            log::error!("No models were provided, make sure to restart with at least one model provided within DKN_MODELS.");
            panic!("No models provided.");
        }

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

    /// Given a raw model name or provider, returns the first matching model.
    ///
    /// If this is a model and is supported by this node, it is returned directly.
    /// If this is a provider, the first matching model in the node config is returned.
    ///
    /// If there are no matching models with this logic, an error is returned.
    pub fn get_matching_model(
        &self,
        model_or_provider: String,
    ) -> Result<(ModelProvider, Model), String> {
        // TODO: use try_from here when Ollama workflows has it
        let model_provider = match model_or_provider.as_str() {
            "ollama" => Some(ModelProvider::Ollama),
            "openai" => Some(ModelProvider::OpenAI),
            _ => None,
        };
        if let Some(model_provider) = model_provider {
            // this is a valid provider, return the first matching model in the config
            self.models
                .iter()
                .find(|(provider, _)| *provider == model_provider)
                .ok_or_else(|| {
                    format!(
                        "No model found for provider {} in the node config.",
                        model_or_provider
                    )
                })
                .cloned()
        } else if let Ok(model) = Model::try_from(model_or_provider.clone()) {
            // this is a valid model, return it if it is supported by the node
            self.models
                .iter()
                .find(|(_, m)| *m == model)
                .ok_or_else(|| {
                    format!("Model {} is not supported by this node.", model_or_provider)
                })
                .cloned()
        } else {
            // this is neither a valid provider or model for this node
            return Err(format!(
                "Given string '{}' is neither a model nor provider.",
                model_or_provider
            ));
        }
    }

    /// Check if the required compute services are running, e.g. if Ollama
    /// is detected as a provider for the chosen models, it will check that
    /// Ollama is running.
    pub async fn check_services(&self) -> Result<(), String> {
        log::info!("Checking configured services.");
        let unique_providers: Vec<ModelProvider> =
            self.models
                .iter()
                .fold(Vec::new(), |mut unique, (provider, _)| {
                    if !unique.contains(provider) {
                        unique.push(provider.clone());
                    }
                    unique
                });

        // if Ollama is a provider, check that it is running & Ollama models are pulled (or pull them)
        if unique_providers.contains(&ModelProvider::Ollama) {
            self.ollama
                .check(
                    self.models
                        .iter()
                        .filter_map(|(provider, model)| {
                            if *provider == ModelProvider::Ollama {
                                Some(model.to_string())
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
                .await?;
        }

        // if OpenAI is a provider, check that the API key is set
        if unique_providers.contains(&ModelProvider::OpenAI) {
            log::info!("Checking OpenAI requirements");
            const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

            if std::env::var(OPENAI_API_KEY).is_err() {
                return Err("OpenAI API key not found".into());
            }
        }

        Ok(())
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
        env::set_var("DKN_MODELS", "phi3:3.8b");

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
        let cfg = DriaComputeNodeConfig::new();

        assert_eq!(
            cfg.get_matching_model("openai".to_string()).unwrap().1,
            Model::GPT3_5Turbo,
            "Should find gpt-3.5-turbo"
        );

        assert_eq!(
            cfg.get_matching_model("phi3:3.8b".to_string()).unwrap().1,
            Model::Phi3Mini,
            "Should find phi3:3.8b"
        );

        assert!(
            cfg.get_matching_model("gpt-4o".to_string()).is_err(),
            "Should not find anything for unsupported model"
        );

        assert!(
            cfg.get_matching_model("praise the model".to_string())
                .is_err(),
            "Should not find anything for inexisting model"
        );
    }
}
