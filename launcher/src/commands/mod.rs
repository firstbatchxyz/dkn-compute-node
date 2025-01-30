use clap::Subcommand;

mod env;
pub use env::edit_environment_file;

mod models;
pub use models::edit_models;

mod apikey;
pub use apikey::edit_api_keys;

mod wallet;
pub use wallet::edit_wallet;

/// Compute node commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Configure your wallet (secret key).
    Wallet,
    /// Configure the selected models.
    Models,
    /// Configure your API Keys.
    ApiKeys,
    /// Edit the environment variables in raw mode.
    Env,
    /// Launch the compute node.
    Compute,
}
