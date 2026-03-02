mod config;
mod error;
mod identity;
mod inference;
mod models;
mod network;
mod stats;
mod worker;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use config::{Cli, Command, Config};
use identity::Identity;
use models::{ModelCache, ModelDownloader, default_registry, resolve_model};
use models::registry::ModelSpec;
use network::{NodeMessage, RouterMessage};
use network::protocol::ModelType;
use network::RouterConnection;
use stats::NodeStats;
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

/// Shared state needed by event handlers for reconnection and challenge-response.
struct NodeContext {
    identity: Identity,
    config: Config,
    tps: HashMap<String, f64>,
    stats: Arc<NodeStats>,
    cache: ModelCache,
}

/// Result of a background model download + load operation.
struct ModelLoadResult {
    name: String,
    template: String,
    model_type: ModelType,
    result: Result<(inference::InferenceEngine, f64), error::NodeError>,
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

    // Accumulate engines and TPS per model
    let mut engines: HashMap<String, (inference::InferenceEngine, String, ModelType)> = HashMap::new();
    let mut tps_map: HashMap<String, f64> = HashMap::new();

    for model_name in &config.model_names {
        let spec = resolve_model(model_name, &registry)
            .ok_or_else(|| error::NodeError::Model(format!("unknown model: {model_name}")))?;

        let (engine, tps) = download_and_load_model(&spec, &cache, config.gpu_layers).await?;
        let chat_template = spec
            .chat_template
            .clone()
            .unwrap_or_else(|| "chatml".to_string());

        tracing::info!(tps = %format!("{tps:.1}"), model = %model_name, "benchmark complete");
        engines.insert(model_name.clone(), (engine, chat_template, spec.model_type));
        tps_map.insert(model_name.clone(), tps);
    }

    if engines.is_empty() {
        return Err(error::NodeError::Config("no models loaded".into()).into());
    }

    // Build the worker
    let mut worker = Worker::new(engines, config.max_concurrent);

    // Attempt router connection; try each URL, go offline if all unavailable
    let mut connection: Option<RouterConnection> = None;
    for url in &config.router_urls {
        match RouterConnection::connect(
            url,
            config.insecure,
            &identity,
            config.model_names.clone(),
            tps_map.clone(),
            worker.capacity(),
        )
        .await
        {
            Ok(conn) => {
                tracing::info!(node_id = %conn.node_id, router = %url, "connected to router");
                connection = Some(conn);
                break;
            }
            Err(e) => {
                tracing::warn!(%e, router = %url, "failed to connect to router");
            }
        }
    }
    if connection.is_none() {
        tracing::warn!("all routers unavailable, running in offline mode");
    }

    tracing::info!(
        routers = ?config.router_urls,
        models = ?config.model_names,
        max_concurrent = config.max_concurrent,
        insecure = config.insecure,
        online = connection.is_some(),
        "node ready"
    );

    // Build shared context for event handlers
    let stats = Arc::new(NodeStats::new());
    let mut ctx = NodeContext {
        identity,
        config,
        tps: tps_map,
        stats: Arc::clone(&stats),
        cache,
    };

    // Channel for background model load results
    let (model_tx, mut model_rx) = mpsc::unbounded_channel::<ModelLoadResult>();

    // Main event loop
    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.tick().await; // consume the immediate first tick
    loop {
        let event = tokio::select! {
            msg = recv_router_msg(&mut connection) => Event::RouterMsg(msg),
            Some(done) = worker.next_completed() => Event::TaskDone(done),
            Some(loaded) = model_rx.recv() => Event::ModelLoaded(loaded),
            _ = stats_interval.tick() => Event::StatsLog,
            _ = tokio::signal::ctrl_c() => Event::Shutdown,
        };

        match event {
            Event::RouterMsg(Ok(Some(msg))) => {
                handle_router_message(msg, &mut worker, &mut connection, &mut ctx, &model_tx).await;
            }
            Event::RouterMsg(Ok(None)) => {
                // Stream closed cleanly
                tracing::warn!("router stream closed, attempting reconnect");
                if let Some(ref conn) = connection {
                    conn.close();
                }
                connection = try_reconnect(&ctx, worker.capacity()).await;
            }
            Event::RouterMsg(Err(e)) => {
                tracing::warn!(%e, "router communication error, attempting reconnect");
                if let Some(ref conn) = connection {
                    conn.close();
                }
                connection = try_reconnect(&ctx, worker.capacity()).await;
            }
            Event::TaskDone(completed) => {
                handle_completed_task(completed, &mut connection, &ctx.stats).await;
            }
            Event::ModelLoaded(loaded) => {
                match loaded.result {
                    Ok((engine, tps)) => {
                        tracing::info!(
                            model = %loaded.name,
                            tps = %format!("{tps:.1}"),
                            "model loaded successfully"
                        );
                        worker.add_engine(loaded.name.clone(), engine, loaded.template, loaded.model_type);
                        ctx.tps.insert(loaded.name, tps);
                    }
                    Err(e) => {
                        tracing::error!(model = %loaded.name, %e, "failed to load model");
                    }
                }
            }
            Event::StatsLog => {
                ctx.stats.log_summary();
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
                    handle_completed_task(completed, &mut connection, &ctx.stats).await;
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
    ModelLoaded(ModelLoadResult),
    StatsLog,
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

/// Attempt to reconnect to the router with exponential backoff.
///
/// Tries up to 5 rounds, iterating all router URLs per round (1s → 2s → 4s → 8s → 16s),
/// then gives up and returns None so the main loop can fall back to the offline sleep-and-retry cycle.
async fn try_reconnect(
    ctx: &NodeContext,
    capacity: network::protocol::Capacity,
) -> Option<RouterConnection> {
    let mut delay = Duration::from_secs(1);
    let max_rounds = 5;

    for round in 1..=max_rounds {
        tracing::info!(round, delay_secs = delay.as_secs(), "attempting reconnect");
        tokio::time::sleep(delay).await;

        for url in &ctx.config.router_urls {
            match RouterConnection::connect(
                url,
                ctx.config.insecure,
                &ctx.identity,
                ctx.config.model_names.clone(),
                ctx.tps.clone(),
                capacity.clone(),
            )
            .await
            {
                Ok(conn) => {
                    tracing::info!(node_id = %conn.node_id, router = %url, "reconnected to router");
                    return Some(conn);
                }
                Err(e) => {
                    tracing::warn!(%e, router = %url, round, "reconnect attempt failed");
                }
            }
        }

        delay *= 2;
    }

    tracing::warn!("all reconnect attempts exhausted, running in offline mode");
    None
}

/// Handle a router message: dispatch tasks, respond to pings, sign challenges, etc.
async fn handle_router_message(
    msg: RouterMessage,
    worker: &mut Worker,
    connection: &mut Option<RouterConnection>,
    ctx: &mut NodeContext,
    model_tx: &mpsc::UnboundedSender<ModelLoadResult>,
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
                    ctx.stats.record_rejected();
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
                    models: worker.model_names(),
                    capacity: worker.capacity(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    stats: Some(ctx.stats.snapshot()),
                };
                if let Err(e) = conn.send(&status).await {
                    tracing::error!(%e, "failed to send status update");
                }
            }
        }
        RouterMessage::Challenge { challenge } => {
            tracing::debug!("received challenge, signing response");
            let (sig, recid) = ctx.identity.sign(&challenge);
            if let Some(ref mut conn) = connection {
                let response = NodeMessage::ChallengeResponse {
                    challenge,
                    signature: sig.serialize().to_vec(),
                    recovery_id: recid.serialize(),
                };
                if let Err(e) = conn.send(&response).await {
                    tracing::error!(%e, "failed to send challenge response");
                }
            }
        }
        RouterMessage::ModelRegistryUpdate { entries } => {
            tracing::info!(count = entries.len(), "received model registry update");

            // Compute desired set from entries
            let desired: HashMap<String, _> = entries
                .iter()
                .map(|e| (e.name.clone(), e))
                .collect();

            // Remove models not in the desired set
            let current = worker.model_names();
            for name in &current {
                if !desired.contains_key(name) {
                    tracing::info!(model = %name, "removing model (not in registry)");
                    worker.remove_engine(name);
                    ctx.tps.remove(name);
                }
            }

            // Spawn background download+load for new models
            for entry in &entries {
                if !worker.has_model(&entry.name) {
                    let spec = ModelSpec::from_registry_entry(entry);
                    let cache = ctx.cache.clone();
                    let gpu_layers = ctx.config.gpu_layers;
                    let tx = model_tx.clone();
                    let name = entry.name.clone();
                    let template = entry
                        .chat_template
                        .clone()
                        .unwrap_or_else(|| "chatml".to_string());
                    let model_type = entry.model_type;

                    tracing::info!(model = %name, "spawning background model download+load");
                    tokio::spawn(async move {
                        let result = download_and_load_model(&spec, &cache, gpu_layers).await;
                        let _ = tx.send(ModelLoadResult { name, template, model_type, result });
                    });
                }
            }
        }
    }
}

/// Download (if needed), verify, cache, load, and benchmark a model.
///
/// Returns the loaded engine and its benchmark TPS.
async fn download_and_load_model(
    spec: &ModelSpec,
    cache: &ModelCache,
    gpu_layers: i32,
) -> Result<(inference::InferenceEngine, f64), error::NodeError> {
    let model_name = spec.name.clone();

    // Check local cache first
    let model_path = if let Some(path) = cache.get_local_path(spec) {
        tracing::info!(model = %model_name, path = %path.display(), "model found in cache");
        path
    } else {
        // Download from HuggingFace
        let hf_path = ModelDownloader::download(spec).await?;

        // Verify SHA-256 if specified
        if let Some(ref expected_sha) = spec.sha256 {
            tracing::info!(model = %model_name, "verifying SHA-256");
            if !ModelCache::verify_sha256(&hf_path, expected_sha)? {
                return Err(error::NodeError::Model(format!(
                    "SHA-256 mismatch for model {model_name}"
                )));
            }
        }

        // Link into our cache
        cache.link_model(spec, &hf_path)?
    };

    // Load model and run benchmark in blocking thread
    let (engine, tps) = tokio::task::spawn_blocking(move || {
        let engine = inference::InferenceEngine::load(&model_path, gpu_layers)?;
        let tps_result = engine.benchmark(&model_name)?;
        Ok::<_, error::NodeError>((engine, tps_result.generation_tps))
    })
    .await
    .map_err(|e| error::NodeError::Inference(format!("task join error: {e}")))?
    ?;

    Ok((engine, tps))
}

/// Handle a completed inference task: send result or log if offline.
async fn handle_completed_task(
    completed: CompletedTask,
    connection: &mut Option<RouterConnection>,
    stats: &NodeStats,
) {
    match completed.result {
        Ok(ref msg) => {
            let tokens = match msg {
                NodeMessage::TaskResult { stats: ts, .. } => ts.tokens_generated,
                _ => 0,
            };
            stats.record_completed(tokens);
            tracing::info!(task_id = %completed.task_id, "task completed");
            if let Some(ref mut conn) = connection {
                if let Err(e) = conn.send(msg).await {
                    tracing::error!(%e, task_id = %completed.task_id, "failed to send result");
                }
            } else {
                tracing::warn!(task_id = %completed.task_id, "task completed but offline, result dropped");
            }
        }
        Err(e) => {
            stats.record_failed();
            tracing::error!(%e, task_id = %completed.task_id, "task failed");
        }
    }
}
