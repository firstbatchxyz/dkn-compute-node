use std::env;

use dkn_compute::*;
use eyre::{Context, Result};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();
    // TODO: remove me later when the launcher is fixed
    amend_log_levels();

    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();
    println!("LOG_LEVEL: {}", env::var("RUST_LOG").unwrap());
    if let Err(e) = dotenv_result {
        log::warn!("Could not load .env file: {}", e);
    }

    log::info!(
        r#"

██████╗ ██████╗ ██╗ █████╗ 
██╔══██╗██╔══██╗██║██╔══██╗   Dria Compute Node 
██║  ██║██████╔╝██║███████║   v{DRIA_COMPUTE_NODE_VERSION}
██║  ██║██╔══██╗██║██╔══██║   https://dria.co
██████╔╝██║  ██║██║██║  ██║
╚═════╝ ╚═╝  ╚═╝╚═╝╚═╝  ╚═╝
"#,
    );

    let token = CancellationToken::new();
    let cancellation_token = token.clone();
    tokio::spawn(async move {
        if let Some(timeout_str) = env::var("DKN_EXIT_TIMEOUT").ok() {
            // add cancellation check
            let duration_secs = timeout_str.parse().unwrap_or(120);
            tokio::time::sleep(tokio::time::Duration::from_secs(duration_secs)).await;
            cancellation_token.cancel();
        } else {
            if let Err(err) = wait_for_termination(cancellation_token.clone()).await {
                log::error!("Error waiting for termination: {:?}", err);
                log::error!("Cancelling due to unexpected error.");
                cancellation_token.cancel();
            };
        }
    });

    // create configurations & check required services & address in use
    let mut config = DriaComputeNodeConfig::new();
    config.assert_address_not_in_use()?;
    let service_check_token = token.clone();
    let service_check_handle = tokio::spawn(async move {
        tokio::select! {
            _ = service_check_token.cancelled() => {
                log::info!("Service check cancelled.");
                config
            }
            result = config.workflows.check_services() => {
                if let Err(err) = result {
                    log::error!("Error checking services: {:?}", err);
                    panic!("Service check failed.")
                }
                config
            }
        }
    });
    let config = service_check_handle
        .await
        .wrap_err("error during service checks")?;

    log::warn!("Using models: {:#?}", config.workflows.models);
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

/// Waits for various termination signals, and cancels the given token when the signal is received.
///
/// Handles Unix and Windows [target families](https://doc.rust-lang.org/reference/conditional-compilation.html#target_family).
async fn wait_for_termination(cancellation: CancellationToken) -> Result<()> {
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

/// Very CRUDE fix due to launcher log level bug
///
/// TODO: remove me later when the launcher is fixed
pub fn amend_log_levels() {
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        let log_level = if rust_log.contains("dkn_compute=info") {
            "info"
        } else if rust_log.contains("dkn_compute=debug") {
            "debug"
        } else if rust_log.contains("dkn_compute=trace") {
            "trace"
        } else {
            return;
        };

        // check if it contains other log levels
        let mut new_rust_log = rust_log.clone();
        if !rust_log.contains("dkn_p2p") {
            new_rust_log = format!("{},{}={}", new_rust_log, "dkn_p2p", log_level);
        }
        if !rust_log.contains("dkn_workflows") {
            new_rust_log = format!("{},{}={}", new_rust_log, "dkn_workflows", log_level);
        }
        std::env::set_var("RUST_LOG", new_rust_log);
    } else {
        // TODO: use env_logger default function instead of this
        std::env::set_var(
            "RUST_LOG",
            "none,dkn_compute=info,dkn_p2p=info,dkn_workflows=info",
        );
    }
}
