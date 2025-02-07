use tokio::process::{Child, Command};

pub async fn run_compute() -> std::io::Result<()> {
    let mut child = Command::new("./target/debug/dkn-compute")
        // .stdout(Stdio::null()) // ignore the output for simplicity
        .spawn()?;

    // wait a few seonds
    std::thread::sleep(std::time::Duration::from_secs(5));

    // kill the process
    // kill the process
    if let Err(e) = child.kill().await {
        log::error!("Failed to kill Compute process: {}", e);
    } else {
        log::info!("Ollama process killed.");
    }
    Ok(())
}
