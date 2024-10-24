use eyre::{Context, Result};
use std::env;
use std::process::Stdio;
use tokio::process::{Child, Command};
use which::which;

pub async fn run_ollama() -> Result<Child> {
    // find the path to binary
    let exe_path = which("ollama").wrap_err("could not find Ollama executable")?;

    log::debug!("Using Ollama executable at {:?}", exe_path);

    // ollama requires the OLLAMA_HOST environment variable to be set before launch
    env::set_var("OLLAMA_HOST", "http://127.0.0.1:11434");
    Command::new(exe_path)
        .arg("serve")
        .stdout(Stdio::null()) // Ignore the output for simplicity
        .spawn()
        .wrap_err("could not spawn Ollama")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_run() {
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
}
