use dkn_compute::*;
use eyre::Result;
use std::env;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();

    // TODO: remove me later when the launcher is fixed
    amend_log_levels();

    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();
    if let Err(e) = dotenv_result {
        log::warn!("could not load .env file: {}", e);
    }

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

    // task tracker for multiple threads
    let task_tracker = TaskTracker::new();
    let cancellation = CancellationToken::new();

    // spawn the background task to wait for termination signals
    let task_tracker_to_close = task_tracker.clone();
    let cancellation_token = cancellation.clone();
    tokio::spawn(async move {
        if let Ok(Ok(duration_secs)) =
            env::var("DKN_EXIT_TIMEOUT").map(|s| s.to_string().parse::<u64>())
        {
            // the timeout is done for profiling only, and should not be used in production
            log::warn!("Waiting for {} seconds before exiting.", duration_secs);
            tokio::time::sleep(tokio::time::Duration::from_secs(duration_secs)).await;

            log::warn!("Exiting due to DKN_EXIT_TIMEOUT.");

            cancellation_token.cancel();
        } else if let Err(err) = wait_for_termination(cancellation_token.clone()).await {
            // if there is no timeout, we wait for termination signals here
            log::error!("Error waiting for termination: {:?}", err);
            log::error!("Cancelling due to unexpected error.");
            cancellation_token.cancel();
        };

        // close tracker in any case
        task_tracker_to_close.close();
    });

    // create configurations & check required services & address in use
    let mut config = DriaComputeNodeConfig::new();
    config.assert_address_not_in_use()?;
    // check services & models, will exit if there is an error
    // since service check can take time, we allow early-exit here as well
    tokio::select! {
        result = config.workflows.check_services() => result,
        _ = cancellation.cancelled() => {
            log::info!("Service check cancelled, exiting.");
            return Ok(());
        }
    }?;
    log::warn!("Using models: {:#?}", config.workflows.models);

    // create the node
    let (mut node, p2p, worker_batch, worker_single) = DriaComputeNode::new(config).await?;

    // spawn threads
    log::info!("Spawning peer-to-peer client thread.");
    task_tracker.spawn(async move { p2p.run().await });

    if let Some(mut worker_batch) = worker_batch {
        log::info!("Spawning workflows batch worker thread.");
        task_tracker.spawn(async move { worker_batch.run_batch().await });
    }

    if let Some(mut worker_single) = worker_single {
        log::info!("Spawning workflows single worker thread.");
        task_tracker.spawn(async move { worker_single.run().await });
    }

    // launch the node in a separate thread
    log::info!("Spawning compute node thread.");
    let node_token = cancellation.clone();
    task_tracker.spawn(async move {
        if let Err(err) = node.run(node_token).await {
            log::error!("Node launch error: {}", err);
            panic!("Node failed.")
        };
        log::info!("Closing node.")
    });

    // wait for all tasks to finish
    task_tracker.wait().await;
    log::info!("All tasks have exited succesfully.");

    log::info!("Bye!");
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

        cancellation.cancel();
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

        cancellation.cancel();
    }

    #[cfg(not(any(unix, windows)))]
    {
        log::error!("No signal handling for this platform: {}", env::consts::OS);
        cancellation.cancel();
    }

    log::info!("Terminating the application...");

    Ok(())
}

// #[deprecated]
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
