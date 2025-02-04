use clap::Subcommand;

mod compute;

mod editor;
pub use editor::edit_environment_file;

mod settings;
pub use settings::change_settings;

/// Compute node commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Change node settings: models, api keys, network settings.
    Settings,
    /// Launch the compute node.
    Compute,
    /// Open a command-line text editor for your environment file (advanced).
    EnvEditor,
}
