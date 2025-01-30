use eyre::{Context, Result};
use std::env;
use std::process::Stdio;
use tokio::process::{Child, Command};
use which::which;

/// Launches a local Ollama server at the given host and port.
///
/// ## Arguments
/// - `host`: The host to bind the server to, usually `http://127.0.0.1`
/// - `port`: The port to bind the server to, usually `11434`
///
/// ## Returns
/// A `Child` process handle to the spawned Ollama process.
///
/// ## Errors
/// - If the Ollama executable is not found in the system.
pub async fn run_ollama(host: &str, port: u16) -> Result<Child> {
    // find the path to binary
    let exe_path = which("ollama").wrap_err("could not find Ollama executable")?;

    log::debug!("Using Ollama executable at {:?}", exe_path);

    // ollama requires the OLLAMA_HOST environment variable to be set before launching
    env::set_var("OLLAMA_HOST", format!("{}:{}", host, port));
    let command = Command::new(exe_path)
        .arg("serve")
        .stdout(Stdio::null()) // ignore the output for simplicity
        .spawn()
        .wrap_err("could not spawn Ollama")?;

    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_run() {
        let (host, port) = ("http://127.0.0.1", 11434);
        let mut child = run_ollama(host, port).await.unwrap();

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
}
