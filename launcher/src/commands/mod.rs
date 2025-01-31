use clap::Subcommand;

mod compute;

mod settings;
pub use settings::change_settings;

/// Compute node commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Change node settings.
    Settings,
    /// Launch the compute node.
    Compute,
}
