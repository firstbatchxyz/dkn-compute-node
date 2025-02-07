use clap::Subcommand;

mod compute;
pub use compute::run_compute;

mod editor;
pub use editor::edit_environment_file;

mod settings;
pub use settings::change_settings;

mod version;
pub use version::change_version;

/// Compute node commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Change node settings: models, api keys, network settings.
    Settings,
    /// Launch the compute node.
    Compute,
    /// Open a command-line text editor for your environment file (advanced).
    EnvEditor,
    /// Change active compute node version.
    Version,
}
