use dkn_compute::*;
use dkn_workflows::DriaWorkflowsConfig;
use eyre::Result;
use std::env;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

#[tokio::main]
async fn main() -> Result<()> {
    let dotenv_result = dotenvy::dotenv();

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
        Ok(path) => log::info!("Loaded .env file at: {}", path.display()),
        Err(e) => log::warn!("Could not load .env file: {}", e),
    }

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
    let workflows_config =
        DriaWorkflowsConfig::new_from_csv(&env::var("DKN_MODELS").unwrap_or_default());
    if workflows_config.models.is_empty() {
        return Err(eyre::eyre!("No models were provided, make sure to restart with at least one model provided within DKN_MODELS."));
    }

    log::info!("Configured models: {:?}", workflows_config.models);
    let mut config = DriaComputeNodeConfig::new(workflows_config);
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

    // spawn p2p client first
    log::info!("Spawning peer-to-peer client thread.");
    task_tracker.spawn(async move { p2p.run().await });

    // spawn batch worker thread if we are using such models (e.g. OpenAI, Gemini, OpenRouter)
    if let Some(mut worker_batch) = worker_batch {
        log::info!("Spawning workflows batch worker thread.");
        task_tracker.spawn(async move { worker_batch.run_batch().await });
    }

    // spawn single worker thread if we are using such models (e.g. Ollama)
    if let Some(mut worker_single) = worker_single {
        log::info!("Spawning workflows single worker thread.");
        task_tracker.spawn(async move { worker_single.run().await });
    }

    // spawn compute node thread
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
