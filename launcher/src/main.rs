use dkn_compute::*;
use dkn_launcher::gemini::is_gemini_api_key_required;
use dkn_launcher::openai::is_openai_api_key_required;
use dkn_launcher::openrouter::is_openrouter_api_key_required;
use dkn_workflows::Model;

use eyre::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::{
    env::{self, set_var},
    fs::OpenOptions,
};
use tokio::process::{Child, Command};
use which::which;

use clap::{Parser, Subcommand};
use crossterm::style::{Attribute, Color, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use dotenvy::from_path_iter;
use inquire::{required, validator::MinLengthValidator, MultiSelect, Text};
use self_update::cargo_crate_version;

#[derive(Parser)]
#[command(name = "dkn-launcher", version, about = "Dria Knowledge Network launcher", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // Configure the environment variables for the node.
    Configure {
        #[arg(
            short,
            long,
            help = "Path to the .env file",
            default_value = ".env",
            required = false
        )]
        path: PathBuf,
        #[arg(short, long, help = "Edit all env variables", required = false)]
        all: bool,
    },
    // Launch the compute node.
    Compute {
        #[arg(
            short,
            long,
            help = "Path to the .env file",
            default_value = ".env",
            required = false,
            value_parser = clap::builder::FalseyValueParser::new()
        )]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::try_parse();

    match cli {
        Ok(cli) => {
            match &cli.command {
                Commands::Configure { path, all } => {
                    log::info!("Configuring the environment variables from: {:?}", path);
                    // terminal setup
                    terminal::enable_raw_mode()?;
                    let mut stdout = io::stdout();
                    execute!(stdout, EnterAlternateScreen, cursor::MoveTo(0, 0))?;
                    execute!(
                        stdout,
                        SetForegroundColor(Color::Blue),
                        SetAttribute(Attribute::Italic)
                    )?;

                    if *all {
                        configure_all(&path)?;
                    } else {
                        configure(&path)?;
                    }

                    terminal::disable_raw_mode()?;
                    execute!(stdout, LeaveAlternateScreen)?;
                }
                Commands::Compute { path } => {
                    // update the launcher
                    // self_update().await?;

                    // run models
                    // run_ollama().await?;

                    // launch the node
                    // launch_compute_node(&path).await?;
                }
            };
        }
        Err(e) => log::warn!("Failed to parse command line argument: {}", e),
    }

    Ok(())
}

fn configure(path: &PathBuf) -> Result<()> {
    // read env
    let mut env_vars = from_path_iter(path)
        .expect("Unable to read env")
        .map(|values| values.expect("Unable to map env vars"))
        .collect::<Vec<(String, String)>>();

    // holds selected models to set related api key later
    let mut selected_models: Vec<String>;
    let mut selected_models_str = String::new();

    // TODO: hold models in different arrays according to their providers
    let models = vec![
        Model::NousTheta,
        Model::Phi3Medium,
        Model::Phi3Medium128k,
        Model::Phi3_5Mini,
        Model::Phi3_5MiniFp16,
        Model::Gemma2_9B,
        Model::Gemma2_9BFp16,
        Model::Llama3_1_8B,
        Model::Llama3_1_8Bq8,
        Model::Llama3_1_8Bf16,
        Model::Llama3_1_70B,
        Model::Llama3_1_70Bq8,
        Model::Llama3_2_1B,
        Model::Llama3_2_3B,
        Model::Qwen2_5_7B,
        Model::Qwen2_5_7Bf16,
        Model::Qwen2_5_32Bf16,
        Model::Qwen2_5Coder1_5B,
        Model::Qwen2_5coder7B,
        Model::Qwen2_5oder7Bq8,
        Model::Qwen2_5coder7Bf16,
        Model::DeepSeekCoder6_7B,
        Model::Mixtral8_7b,
        Model::GPT4Turbo,
        Model::GPT4o,
        Model::GPT4oMini,
        Model::O1Preview,
        Model::O1Mini,
        Model::Gemini15ProExp0827,
        Model::Gemini15Pro,
        Model::Gemini15Flash,
        Model::Gemini10Pro,
        Model::Gemma2_2bIt,
        Model::Gemma2_9bIt,
        Model::Gemma2_27bIt,
    ];

    // loop through env vars
    for (key, val) in env_vars.iter_mut() {
        println!("Key: '{}', Value: '{}'", key, val);

        // ask only for the empty values
        if val.is_empty() {
            match key.as_str() {
                "OPENAI_API_KEY" | "GEMINI_API_KEY" | "OPENROUTER_API_KEY" => {
                    continue;
                }
                // ask for the wallet secret key
                "DKN_WALLET_SECRET_KEY" => {
                    let new_secret_key = Text::new(key)
                        .with_validator(required!("Wallet secret key is required"))
                        .prompt();

                    // update the secret key in env_vars
                    if let Ok(new_secret_key) = new_secret_key {
                        *val = new_secret_key.clone();
                        set_var(key, new_secret_key);
                    }
                }
                // at least one model must be selected
                "DKN_MODELS" => {
                    let dkn_models = MultiSelect::new(key, models.clone())
                        .with_validator(MinLengthValidator::new(1))
                        .prompt();

                    if let Ok(dkn_models) = dkn_models {
                        selected_models = dkn_models
                            .into_iter()
                            .map(|model| model.to_string())
                            .collect::<Vec<String>>();

                        selected_models_str = selected_models.join(",");

                        // set the selected models
                        *val = selected_models_str.clone();
                        set_var(key, selected_models_str.clone());
                    }
                }
                _ => {
                    // update value for other env vars
                    let new_value = Text::new(key).prompt();
                    if let Ok(new_value) = new_value {
                        *val = new_value.clone();
                        set_var(key, new_value);
                    }
                }
            }
        }
    }

    // api key setting according to the selected models
    is_openrouter_api_key_required(&selected_models_str, &mut env_vars);
    is_openai_api_key_required(&selected_models_str, &mut env_vars);
    is_gemini_api_key_required(&selected_models_str, &mut env_vars);

    // open file for writing
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("Unable to open file");

    // write new values to the .env file
    for (key, val) in &env_vars {
        writeln!(file, "{}={}", key, val).expect("Unable to write to file");
    }

    Ok(())
}

fn configure_all(path: &PathBuf) -> Result<()> {
    // read env
    let mut env_vars = from_path_iter(path)
        .expect("Unable to read env")
        .map(|values| values.expect("Unable to map env vars"))
        .collect::<Vec<(String, String)>>();

    // holds selected models to set related api key later
    let mut selected_models: Vec<String>;
    let mut selected_models_str = String::new();

    // models
    let models = vec![
        Model::NousTheta,
        Model::Phi3Medium,
        Model::Phi3Medium128k,
        Model::Phi3_5Mini,
        Model::Phi3_5MiniFp16,
        Model::Gemma2_9B,
        Model::Gemma2_9BFp16,
        Model::Llama3_1_8B,
        Model::Llama3_1_8Bq8,
        Model::Llama3_1_8Bf16,
        Model::Llama3_1_70B,
        Model::Llama3_1_70Bq8,
        Model::Llama3_2_1B,
        Model::Llama3_2_3B,
        Model::Qwen2_5_7B,
        Model::Qwen2_5_7Bf16,
        Model::Qwen2_5_32Bf16,
        Model::Qwen2_5Coder1_5B,
        Model::Qwen2_5coder7B,
        Model::Qwen2_5oder7Bq8,
        Model::Qwen2_5coder7Bf16,
        Model::DeepSeekCoder6_7B,
        Model::Mixtral8_7b,
        Model::GPT4Turbo,
        Model::GPT4o,
        Model::GPT4oMini,
        Model::O1Preview,
        Model::O1Mini,
        Model::Gemini15ProExp0827,
        Model::Gemini15Pro,
        Model::Gemini15Flash,
        Model::Gemini10Pro,
        Model::Gemma2_2bIt,
        Model::Gemma2_9bIt,
        Model::Gemma2_27bIt,
    ];

    // loop through env vars
    for (key, val) in env_vars.iter_mut() {
        if val.is_empty() {
            match key.as_str() {
                // skip api keys
                "OPENAI_API_KEY" | "GEMINI_API_KEY" | "OPENROUTER_API_KEY" => {
                    continue;
                }

                // ask for the wallet secret key
                "DKN_WALLET_SECRET_KEY" => {
                    let new_secret_key = Text::new(key)
                        .with_validator(required!("Wallet secret key is required"))
                        .prompt();

                    // update the secret key in env_vars
                    if let Ok(new_secret_key) = new_secret_key {
                        *val = new_secret_key.clone();
                        set_var(key, new_secret_key);
                    }
                }
                // at least one model must be selected
                "DKN_MODELS" => {
                    let dkn_models = MultiSelect::new(key, models.clone())
                        .with_validator(MinLengthValidator::new(1))
                        .prompt();

                    if let Ok(dkn_models) = dkn_models {
                        selected_models = dkn_models
                            .into_iter()
                            .map(|model| model.to_string())
                            .collect::<Vec<String>>();

                        selected_models_str = selected_models.join(",");

                        // set the selected models
                        *val = selected_models_str.clone();
                        set_var(key, selected_models_str.clone());
                    }
                }
                _ => {
                    // for other values
                    let new_value = Text::new(key).prompt();
                    if let Ok(new_value) = new_value {
                        *val = new_value.clone();
                        set_var(key, new_value);
                    }
                }
            }
        }
    }

    // api key setting according to the selected models
    is_openrouter_api_key_required(&selected_models_str, &mut env_vars);
    is_openai_api_key_required(&selected_models_str, &mut env_vars);
    is_gemini_api_key_required(&selected_models_str, &mut env_vars);

    // open file for writing
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true) // clear the file before writing
        .open(path)
        .expect("Unable to open file");

    // write new values to the .env file
    for (key, val) in &env_vars {
        writeln!(file, "{}={}", key, val).expect("Unable to write to file");
    }

    Ok(())
}

pub async fn run_ollama() -> Result<Child> {
    // find the path to binary
    let exe_path = which("ollama").wrap_err("could not find Ollama executable")?;

    log::debug!("Using Ollama executable at {:?}", exe_path);

    // ollama requires the OLLAMA_HOST environment variable to be set before launch
    env::set_var("OLLAMA_HOST", "http://127.0.0.1:11434");
    let command = Command::new(exe_path)
        .arg("serve")
        .stdout(Stdio::null()) // Ignore the output for simplicity
        // if ollama arent running in the background
        .spawn()
        .wrap_err("could not spawn Ollama")?;
    // set host later to default value

    Ok(command)
}

async fn launch_compute_node(path: &PathBuf) -> Result<()> {
    let dotenv_result = dotenvy::from_path(path);

    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .filter(None, log::LevelFilter::Off)
        .filter_module("dkn_compute", log::LevelFilter::Info)
        .filter_module("dkn_p2p", log::LevelFilter::Info)
        .filter_module("dkn_workflows", log::LevelFilter::Info)
        .parse_default_env() // reads RUST_LOG variable
        .init();

    log::info!(
        r#"

██████╗ ██████╗ ██╗ █████╗ 
██╔══██╗██╔══██╗██║██╔══██╗   Dria Compute Node 
██║  ██║██████╔╝██║███████║   v{DRIA_COMPUTE_NODE_VERSION}
██║  ██║██╔══██╗██║██╔══██║   https://dria.co
██████╔╝██║  ██║██║██║  ██║
╚═════╝ ╚═╝  ╚═╝╚═╝╚═╝  ╚═╝
"#
    );

    // log about env usage
    match dotenv_result {
        Ok(path) => log::info!("Loaded .env file at: {:?}", path),
        Err(e) => log::warn!("Could not load .env file: {}", e),
    }

    // launch the compute node
    let _ = launch().await;

    Ok(())
}

async fn self_update() -> Result<()> {
    // TODO: get latest release from github
    let status = self_update::backends::github::Update::configure()
        .repo_owner("firstbatchxyz")
        .repo_name("dkn-launcher")
        .show_output(true)
        .current_version(cargo_crate_version!())
        .build()
        .expect("Unable to build update")
        .update();

    match status {
        Ok(status) => log::info!("Launcher updated with status: {}", status.version()),
        Err(e) => log::warn!("Failed to update: {}", e),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_run_models() {
        let mut child = run_ollama().await.unwrap();

        // wait for 10 seconds
        println!("Waiting for 10 seconds...");
        sleep(Duration::from_secs(10)).await;

        // kill the process
        if let Err(e) = child.kill().await {
            log::error!("Failed to kill Ollama process: {}", e);
        } else {
            log::info!("Ollama process killed.");
        }
    }

    #[test]
    fn test_cli_parse_configure() {
        let args = vec!["dkn-launcher", "configure"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Configure { path, all: _ } => {
                assert_eq!(path, PathBuf::from(".env"));
            }
            _ => panic!("Expected Configure command"),
        }
    }

    #[test]
    fn test_cli_parse_configure_all() {
        let args = vec!["dkn-launcher", "configure", "--all"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Configure { path, all: _ } => {
                assert_eq!(path, PathBuf::from(".env"));
            }
            _ => panic!("Expected Configure command"),
        }
    }

    #[test]
    fn test_configure() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let path = temp_file.path().to_path_buf();

        // Write test env content
        fs::write(&path, "DKN_WALLET_SECRET_KEY=\nDKN_MODELS=\n")?;

        // Mock terminal input
        let _ = configure(&path)?;

        // Read updated env file
        let content = fs::read_to_string(&path)?;
        assert!(content.contains("DKN_WALLET_SECRET_KEY="));
        assert!(content.contains("DKN_MODELS="));

        Ok(())
    }
}
