// Suppress dead-code warnings for public APIs not yet wired to networking.
#![allow(dead_code)]

mod config;
mod error;
mod identity;
mod inference;
mod models;
mod network;
mod worker;

use std::time::Duration;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use config::{Cli, Command, Config};
use identity::Identity;
use models::{ModelCache, ModelDownloader, default_registry, resolve_model};
use network::{NodeMessage, RouterMessage};
use network::RouterConnection;
use worker::{CompletedTask, Worker};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Start {
            wallet,
            model,
            router_url,
            gpu_layers,
            max_concurrent,
            data_dir,
            insecure,
        } => {
            run_start(wallet, model, router_url, gpu_layers, max_concurrent, data_dir, insecure).await?;
        }
    }

    Ok(())
}

async fn run_start(
    wallet: String,
    model: String,
    router_url: String,
    gpu_layers: i32,
    max_concurrent: usize,
    data_dir: Option<std::path::PathBuf>,
    insecure: bool,
) -> anyhow::Result<()> {
    // Parse config
    let config = Config::from_start_args(wallet, model, router_url, gpu_layers, max_concurrent, data_dir, insecure)?;

    // Create identity
    let identity = Identity::from_secret_hex(&config.secret_key_hex)?;
    tracing::info!(address = %format!("0x{}", identity.address_hex), "node identity");

    // Ensure directories exist
    std::fs::create_dir_all(&config.data_dir)?;
    std::fs::create_dir_all(&config.models_dir)?;

    // Resolve and download models
    let registry = default_registry();
    let cache = ModelCache::new(config.models_dir.clone())?;

    // We need to keep one engine alive for inference; use the first model.
    let mut chat_template = "chatml".to_string();
    let mut engine_and_tps: Option<(inference::InferenceEngine, f64)> = None;

    for model_name in &config.model_names {
        let spec = resolve_model(model_name, &registry)
            .ok_or_else(|| error::NodeError::Model(format!("unknown model: {model_name}")))?;

        // Check local cache first
        let model_path = if let Some(path) = cache.get_local_path(&spec) {
            tracing::info!(model = %model_name, path = %path.display(), "model found in cache");
            path
        } else {
            // Download from HuggingFace
            let hf_path = ModelDownloader::download(&spec).await?;

            // Verify SHA-256 if specified
            if let Some(ref expected_sha) = spec.sha256 {
                tracing::info!(model = %model_name, "verifying SHA-256");
                if !ModelCache::verify_sha256(&hf_path, expected_sha)? {
                    anyhow::bail!("SHA-256 mismatch for model {model_name}");
                }
            }

            // Link into our cache
            cache.link_model(&spec, &hf_path)?
        };

        // Remember chat template from the spec
        if let Some(ref tmpl) = spec.chat_template {
            chat_template = tmpl.clone();
        }

        // Load model and run benchmark in blocking thread
        let model_name_owned = model_name.clone();
        let gpu = config.gpu_layers;
        let (engine, tps) = tokio::task::spawn_blocking(move || {
            let engine = inference::InferenceEngine::load(&model_path, gpu)?;
            let tps_result = engine.benchmark(&model_name_owned)?;
            Ok::<_, error::NodeError>((engine, tps_result.generation_tps))
        })
        .await??;

        tracing::info!(tps = %format!("{tps:.1}"), model = %model_name, "benchmark complete");
        engine_and_tps = Some((engine, tps));
    }

    let (engine, tps) = engine_and_tps.ok_or_else(|| {
        error::NodeError::Config("no models loaded".into())
    })?;

    // Build the worker
    let mut worker = Worker::new(
        engine,
        chat_template,
        config.model_names.clone(),
        config.max_concurrent,
    );

    // Attempt router connection; go offline if unavailable
    let mut connection: Option<RouterConnection> = match RouterConnection::connect(
        &config.router_url,
        config.insecure,
        &identity,
        config.model_names.clone(),
        tps,
        worker.capacity(),
    )
    .await
    {
        Ok(conn) => {
            tracing::info!(node_id = %conn.node_id, "connected to router");
            Some(conn)
        }
        Err(e) => {
            tracing::warn!(%e, "failed to connect to router, running in offline mode");
            None
        }
    };

    tracing::info!(
        router = %config.router_url,
        models = ?config.model_names,
        max_concurrent = config.max_concurrent,
        insecure = config.insecure,
        online = connection.is_some(),
        "node ready"
    );

    // Main event loop
    loop {
        let event = tokio::select! {
            msg = recv_router_msg(&mut connection) => Event::RouterMsg(msg),
            Some(done) = worker.next_completed() => Event::TaskDone(done),
            _ = tokio::signal::ctrl_c() => Event::Shutdown,
        };

        match event {
            Event::RouterMsg(Ok(Some(msg))) => {
                handle_router_message(msg, &mut worker, &mut connection).await;
            }
            Event::RouterMsg(Ok(None)) => {
                // Stream closed cleanly
                tracing::warn!("router stream closed, switching to offline mode");
                if let Some(ref conn) = connection {
                    conn.close();
                }
                connection = None;
            }
            Event::RouterMsg(Err(e)) => {
                tracing::warn!(%e, "router communication error");
                if let Some(ref conn) = connection {
                    conn.close();
                }
                connection = None;

                // Attempt reconnect
                tracing::info!("will attempt reconnect on next cycle");
            }
            Event::TaskDone(completed) => {
                handle_completed_task(completed, &mut connection).await;
            }
            Event::Shutdown => {
                tracing::info!("shutdown signal received");
                break;
            }
        }
    }

    // Graceful shutdown: drain in-flight tasks with 30s timeout
    if worker.has_in_flight() {
        tracing::info!("draining in-flight tasks (30s timeout)");
        let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(30);

        loop {
            tokio::select! {
                Some(completed) = worker.next_completed() => {
                    handle_completed_task(completed, &mut connection).await;
                }
                _ = tokio::time::sleep_until(drain_deadline) => {
                    tracing::warn!("drain timeout reached, dropping remaining tasks");
                    break;
                }
            }
            if !worker.has_in_flight() {
                break;
            }
        }
    }

    if let Some(ref conn) = connection {
        conn.close();
    }
    tracing::info!("shutdown complete");

    Ok(())
}

// ---------------------------------------------------------------------------
// Event types for the select! loop
// ---------------------------------------------------------------------------

enum Event {
    RouterMsg(Result<Option<RouterMessage>, error::NodeError>),
    TaskDone(CompletedTask),
    Shutdown,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Receive a router message, or sleep 10s when offline (to allow periodic reconnect).
async fn recv_router_msg(
    connection: &mut Option<RouterConnection>,
) -> Result<Option<RouterMessage>, error::NodeError> {
    match connection {
        Some(ref mut conn) => conn.recv().await,
        None => {
            // Offline: sleep then signal a reconnect attempt
            tokio::time::sleep(Duration::from_secs(10)).await;
            Err(error::NodeError::Network("offline, attempting reconnect".into()))
        }
    }
}

/// Handle a router message: dispatch tasks, respond to pings, etc.
async fn handle_router_message(
    msg: RouterMessage,
    worker: &mut Worker,
    connection: &mut Option<RouterConnection>,
) {
    match msg {
        RouterMessage::TaskAssignment {
            task_id,
            model,
            messages,
            max_tokens,
            temperature,
            validation,
        } => {
            tracing::info!(%task_id, %model, "received task assignment");
            match worker.try_accept(task_id, &model, messages, max_tokens, temperature, validation)
            {
                Ok(()) => {
                    tracing::debug!(%task_id, "task accepted");
                }
                Err(reason) => {
                    tracing::warn!(%task_id, ?reason, "task rejected");
                    if let Some(ref mut conn) = connection {
                        let reject = NodeMessage::TaskRejected { task_id, reason };
                        if let Err(e) = conn.send(&reject).await {
                            tracing::error!(%e, "failed to send rejection");
                        }
                    }
                }
            }
        }
        RouterMessage::Ping => {
            tracing::debug!("received ping");
            if let Some(ref mut conn) = connection {
                let status = NodeMessage::StatusUpdate {
                    models: worker.model_names().to_vec(),
                    capacity: worker.capacity(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                };
                if let Err(e) = conn.send(&status).await {
                    tracing::error!(%e, "failed to send status update");
                }
            }
        }
        RouterMessage::Challenge { challenge } => {
            tracing::debug!(?challenge, "received challenge (not yet implemented)");
            // TODO: implement challenge-response
        }
        RouterMessage::ModelRegistryUpdate { entries } => {
            tracing::info!(count = entries.len(), "received model registry update (not yet implemented)");
            // TODO: handle model registry updates
        }
    }
}

/// Handle a completed inference task: send result or log if offline.
async fn handle_completed_task(
    completed: CompletedTask,
    connection: &mut Option<RouterConnection>,
) {
    match completed.result {
        Ok(msg) => {
            tracing::info!(task_id = %completed.task_id, "task completed");
            if let Some(ref mut conn) = connection {
                if let Err(e) = conn.send(&msg).await {
                    tracing::error!(%e, task_id = %completed.task_id, "failed to send result");
                }
            } else {
                tracing::warn!(task_id = %completed.task_id, "task completed but offline, result dropped");
            }
        }
        Err(e) => {
            tracing::error!(%e, task_id = %completed.task_id, "task failed");
        }
    }
}
