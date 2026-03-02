use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::error::NodeError;

#[derive(Parser)]
#[command(name = "dria-node", version, about = "Dria Compute Node")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the compute node
    Start {
        /// Wallet secret key (hex-encoded, 32 bytes)
        #[arg(long, env = "DRIA_WALLET")]
        wallet: String,

        /// Model(s) to serve (comma-separated shortnames, e.g. "gemma3:4b,llama3.1:8b")
        #[arg(long, env = "DRIA_MODELS")]
        model: String,

        /// Router URL for task coordination
        #[arg(long, env = "DRIA_ROUTER_URL", default_value = "https://router.dria.co")]
        router_url: String,

        /// Number of GPU layers to offload (-1 = all, 0 = CPU only)
        #[arg(long, env = "DRIA_GPU_LAYERS", default_value = "0")]
        gpu_layers: i32,

        /// Maximum concurrent inference requests
        #[arg(long, env = "DRIA_MAX_CONCURRENT", default_value = "1")]
        max_concurrent: usize,

        /// Data directory
        #[arg(long, env = "DRIA_DATA_DIR")]
        data_dir: Option<PathBuf>,

        /// Skip TLS certificate verification (for development/testing)
        #[arg(long, env = "DRIA_INSECURE")]
        insecure: bool,
    },
}

/// Parsed and validated configuration for the node.
pub struct Config {
    pub secret_key_hex: String,
    pub model_names: Vec<String>,
    pub router_url: String,
    pub gpu_layers: i32,
    pub max_concurrent: usize,
    pub data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub insecure: bool,
}

impl Config {
    /// Create a Config from the `start` subcommand arguments.
    pub fn from_start_args(
        wallet: String,
        model: String,
        router_url: String,
        gpu_layers: i32,
        max_concurrent: usize,
        data_dir: Option<PathBuf>,
        insecure: bool,
    ) -> Result<Self, NodeError> {
        // Validate wallet key
        let secret_key_hex = wallet.strip_prefix("0x").unwrap_or(&wallet).to_string();
        if secret_key_hex.len() != 64 {
            return Err(NodeError::Config(format!(
                "wallet secret key must be 64 hex chars, got {}",
                secret_key_hex.len()
            )));
        }
        hex::decode(&secret_key_hex)
            .map_err(|e| NodeError::Config(format!("wallet key is not valid hex: {e}")))?;

        // Parse model names
        let model_names: Vec<String> = model
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if model_names.is_empty() {
            return Err(NodeError::Config("at least one model must be specified".into()));
        }

        // Resolve data directory
        let data_dir = match data_dir {
            Some(d) => d,
            None => dirs::home_dir()
                .ok_or_else(|| NodeError::Config("could not determine home directory".into()))?
                .join(".dria"),
        };
        let models_dir = data_dir.join("models");

        if max_concurrent == 0 {
            return Err(NodeError::Config("max-concurrent must be >= 1".into()));
        }

        Ok(Config {
            secret_key_hex,
            model_names,
            router_url,
            gpu_layers,
            max_concurrent,
            data_dir,
            models_dir,
            insecure,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_valid_args() {
        let cfg = Config::from_start_args(
            "0x6472696164726961647269616472696164726961647269616472696164726961".into(),
            "gemma3:4b, llama3.1:8b".into(),
            "https://router.dria.co".into(),
            0,
            1,
            Some("/tmp/dria-test".into()),
            false,
        )
        .unwrap();

        assert_eq!(cfg.model_names, vec!["gemma3:4b", "llama3.1:8b"]);
        assert_eq!(
            cfg.secret_key_hex,
            "6472696164726961647269616472696164726961647269616472696164726961"
        );
        assert_eq!(cfg.models_dir, PathBuf::from("/tmp/dria-test/models"));
    }

    #[test]
    fn test_config_invalid_wallet_length() {
        let result = Config::from_start_args(
            "0xabcd".into(),
            "gemma3:4b".into(),
            "https://router.dria.co".into(),
            0,
            1,
            None,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_config_invalid_wallet_hex() {
        let result = Config::from_start_args(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".into(),
            "gemma3:4b".into(),
            "https://router.dria.co".into(),
            0,
            1,
            None,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_config_empty_model() {
        let result = Config::from_start_args(
            "6472696164726961647269616472696164726961647269616472696164726961".into(),
            "".into(),
            "https://router.dria.co".into(),
            0,
            1,
            None,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_config_zero_concurrency() {
        let result = Config::from_start_args(
            "6472696164726961647269616472696164726961647269616472696164726961".into(),
            "gemma3:4b".into(),
            "https://router.dria.co".into(),
            0,
            0,
            None,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_config_insecure_flag() {
        let cfg = Config::from_start_args(
            "6472696164726961647269616472696164726961647269616472696164726961".into(),
            "gemma3:4b".into(),
            "https://router.dria.co".into(),
            0,
            1,
            None,
            true,
        )
        .unwrap();
        assert!(cfg.insecure);
    }
}
