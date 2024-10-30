mod ollama;

mod compute;
use compute::*;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dkn", version)]
#[command(about = "Dria Knowledge Network launcher", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure the environment variables for the node.
    Configure {
        #[arg(short, long, help = "Path to the .env file", default_value = ".env")]
        path: PathBuf,
    },
    /// Launch the compute node.
    Compute {
        #[arg(short, long, help = "Path to the .env file", default_value = ".env")]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Configure { path } => {
            println!("Configuring the environment variables from: {:?}", path);
            let iter = dotenvy::from_path_iter(&path)?;

            for line in iter {
                let (key, value) = line?;
                println!("- {}={}", key, value);
            }

            // TODO: use the key-value pairs to set the environment variables
        }
        Commands::Compute { path } => {
            launch_compute_node(path).await?;
        }
    };

    Ok(())
}
