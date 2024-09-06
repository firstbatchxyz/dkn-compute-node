use dkn_compute::{DriaComputeNode, DriaComputeNodeConfig};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = dotenvy::dotenv() {
        log::warn!("Could not load .env file: {}", e);
    }

    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();

    log::info!(
        r#"

██████╗ ██████╗ ██╗ █████╗ 
██╔══██╗██╔══██╗██║██╔══██╗   Dria Compute Node 
██║  ██║██████╔╝██║███████║   v{}
██║  ██║██╔══██╗██║██╔══██║   https://dria.co
██████╔╝██║  ██║██║██║  ██║
╚═════╝ ╚═╝  ╚═╝╚═╝╚═╝  ╚═╝
"#,
        dkn_compute::DRIA_COMPUTE_NODE_VERSION
    );

    let token = CancellationToken::new();
    let cancellation_token = token.clone();
    // add cancellation check
    tokio::spawn(async move {
        // FIXME: weird feature-gating here bugs with IDE, fix this later
        #[cfg(feature = "profiling")]
        {
            const PROFILE_DURATION_SECS: u64 = 120;
            tokio::time::sleep(tokio::time::Duration::from_secs(PROFILE_DURATION_SECS)).await;
            cancellation_token.cancel();
        }

        #[cfg(not(feature = "profiling"))]
        if let Err(err) = wait_for_termination(cancellation_token.clone()).await {
            log::error!("Error waiting for termination: {}", err);
            log::error!("Cancelling due to unexpected error.");
            cancellation_token.cancel();
        };
    });

    // create configurations & check required services
    let config = DriaComputeNodeConfig::new();
    let service_check_token = token.clone();
    let mut config_clone = config.clone();
    let service_check_handle = tokio::spawn(async move {
        tokio::select! {
            _ = service_check_token.cancelled() => {
                log::info!("Service check cancelled.");
            }
            result = config_clone.check_services() => {
                if let Err(err) = result {
                    log::error!("Error checking services: {}", err);
                    panic!("Service check failed.")
                }
            }
        }
    });

    // wait for service check to complete
    if let Err(err) = service_check_handle.await {
        log::error!("Service check handle error: {}", err);
        panic!("Could not exit service check thread handle.");
    };

    if !token.is_cancelled() {
        // launch the node
        let node_token = token.clone();
        let node_handle = tokio::spawn(async move {
            match DriaComputeNode::new(config, node_token).await {
                Ok(mut node) => {
                    if let Err(err) = node.launch().await {
                        log::error!("Node launch error: {}", err);
                        panic!("Node failed.")
                    };
                }
                Err(err) => {
                    log::error!("Node setup error: {}", err);
                    panic!("Could not setup node.")
                }
            }
        });

        // wait for tasks to complete
        if let Err(err) = node_handle.await {
            log::error!("Node handle error: {}", err);
            panic!("Could not exit Node thread handle.");
        };
    }

    Ok(())
}

// FIXME: remove this `unused` once we have a better way to handle this
/// Waits for various termination signals, and cancels the given token when the signal is received.
///
/// Handles Unix and Windows [target families](https://doc.rust-lang.org/reference/conditional-compilation.html#target_family).
#[allow(unused)]
async fn wait_for_termination(cancellation: CancellationToken) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?; // Docker sends SIGTERM
        let mut sigint = signal(SignalKind::interrupt())?; // Ctrl+C sends SIGINT
        tokio::select! {
            _ = sigterm.recv() => log::warn!("Recieved SIGTERM"),
            _ = sigint.recv() => log::warn!("Recieved SIGINT"),
            _ = cancellation.cancelled() => {
                // no need to wait if cancelled anyways
                // although this is not likely to happen
                return Ok(());
            }
        };
    }

    #[cfg(windows)]
    {
        use tokio::signal::windows;

        // https://learn.microsoft.com/en-us/windows/console/handlerroutine
        let mut signal_c = windows::ctrl_c()?;
        let mut signal_break = windows::ctrl_break()?;
        let mut signal_close = windows::ctrl_close()?;
        let mut signal_shutdown = windows::ctrl_shutdown()?;

        tokio::select! {
            _ = signal_c.recv() => log::warn!("Received CTRL_C"),
            _ = signal_break.recv() => log::warn!("Received CTRL_BREAK"),
            _ = signal_close.recv() => log::warn!("Received CTRL_CLOSE"),
            _ = signal_shutdown.recv() => log::warn!("Received CTRL_SHUTDOWN"),
            _ = cancellation.cancelled() => {
                // no need to wait if cancelled anyways
                // although this is not likely to happen
                return Ok(());
            }
        };
    }

    log::info!("Terminating the node...");
    cancellation.cancel();
    Ok(())
}
