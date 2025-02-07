use clap::Parser;
use std::path::PathBuf;

mod commands;
use commands::Commands;

mod settings;

mod env;
pub use env::DriaEnv;

mod signal;
pub use signal::wait_for_termination;

mod utils;

#[derive(Parser)]
#[command(name = "dkn", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the .env file.
    #[arg(short, long, default_value = "./.env")]
    pub env: PathBuf,

    /// Enable debug-level logs
    #[arg(short, long)]
    pub debug: bool,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // default commands such as version and help exit at this point
    let cli = Cli::parse();

    // read env w.r.t cli argument, defaults to `.env`
    let dotenv_result = dotenvy::from_path(&cli.env);

    // init env logger
    let log_level = match cli.debug {
        true => log::LevelFilter::Debug,
        false => log::LevelFilter::Info,
    };
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .filter(None, log::LevelFilter::Off)
        .filter_module("dkn_compute", log_level)
        .filter_module("dkn_launcher", log_level)
        .parse_default_env()
        .init();

    // log about env usage after env logger init is executed
    match dotenv_result {
        Ok(_) => log::info!("Loaded .env file at: {}", cli.env.display()),
        Err(e) => log::warn!("Could not load .env file: {}", e),
    }

    // let env_file = dotenvy::from_path_iter(&cli.env)?;

    match &cli.command {
        Commands::Settings => commands::change_settings(&cli.env)?,
        Commands::EnvEditor => commands::edit_environment_file(&cli.env)?,
        Commands::Version => commands::change_version().await?,
        Commands::Compute => {
            commands::run_compute().await?;
        }
    };

    Ok(())
}
