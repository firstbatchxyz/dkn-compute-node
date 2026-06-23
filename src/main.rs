mod config;
mod error;
mod identity;
mod network;
// The pipeline engine is built ahead of its consumer (the Luminal worker, next milestone).
#[allow(dead_code)]
mod pipeline;
mod setup;
mod stats;
mod update;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use config::{Cli, Command, Config};
use identity::Identity;
use network::protocol::Capacity;
use network::RouterConnection;
use network::{NodeMessage, RouterMessage};
use stats::NodeStats;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Setup {
            data_dir,
            gpu_layers,
        } => {
            setup::run_setup(data_dir, gpu_layers).await?;
        }
        Command::Start {
            wallet,
            model,
            router_url,
            gpu_layers,
            max_concurrent,
            data_dir,
            quant,
            insecure,
            skip_update,
            context_size,
            kv_quant,
        } => {
            run_start(wallet, model, router_url, gpu_layers, max_concurrent, data_dir, quant, insecure, skip_update, context_size, kv_quant).await?;
        }
    }

    Ok(())
}

/// All-Luminal GLM-5.2 rewrite in progress: the inference engine (Luminal pipeline) is not
/// yet wired, so the node connects to the router and advertises **zero** capacity until the
/// engine lands. See `src/pipeline/` and `docs/substrate-decision.md`.
fn placeholder_capacity() -> Capacity {
    Capacity { free: 0, max: 0 }
}

/// Shared state needed by event handlers for reconnection and challenge-response.
struct NodeContext {
    identity: Identity,
    config: Config,
    tps: HashMap<String, f64>,
    stats: Arc<NodeStats>,
}

#[allow(clippy::too_many_arguments)]
async fn run_start(
    wallet: String,
    model: String,
    router_url: String,
    gpu_layers: i32,
    max_concurrent: usize,
    data_dir: Option<std::path::PathBuf>,
    quant: Option<String>,
    insecure: bool,
    skip_update: bool,
    max_context: Option<u32>,
    kv_quant: String,
) -> anyhow::Result<()> {
    let config = Config::from_start_args(wallet, model, router_url, gpu_layers, max_concurrent, data_dir, quant, insecure, skip_update, max_context, kv_quant)?;

    let identity = Identity::from_secret_hex(&config.secret_key_hex)?;
    tracing::info!(address = %format!("0x{}", identity.address_hex), "node identity");

    // Check for updates
    if !config.skip_update {
        match update::check_for_update().await {
            Ok(update::UpdateAction::Force(version)) => {
                tracing::warn!(%version, "mandatory update available, downloading...");
                if let Err(e) = update::perform_update(&version).await {
                    tracing::error!(%e, "auto-update failed, continuing with current version");
                } else {
                    tracing::info!("update complete — please restart the node");
                    return Ok(());
                }
            }
            Ok(update::UpdateAction::Warn(version)) => {
                tracing::warn!(%version, "new patch version available, update recommended (current: {})", env!("CARGO_PKG_VERSION"));
            }
            Ok(update::UpdateAction::UpToDate) => {}
            Err(e) => tracing::debug!(%e, "update check failed, continuing"),
        }
    }

    std::fs::create_dir_all(&config.data_dir)?;

    eprint!("{}", include_str!("../dnet.art"));

    // Inference engine (Luminal GLM-5.2 pipeline) not yet wired — connect with no capacity.
    let tps_map: HashMap<String, f64> = HashMap::new();
    let mut connection: Option<RouterConnection> = None;
    for url in &config.router_urls {
        match RouterConnection::connect(url, config.insecure, &identity, config.model_names.clone(), tps_map.clone(), placeholder_capacity()).await {
            Ok(conn) => {
                tracing::info!(node_id = %conn.node_id, router = %url, "connected to router");
                connection = Some(conn);
                break;
            }
            Err(e) => tracing::warn!(%e, router = %url, "failed to connect to router"),
        }
    }
    if connection.is_none() {
        tracing::warn!("all routers unavailable, running in offline mode");
    }

    tracing::info!(
        routers = ?config.router_urls,
        models = ?config.model_names,
        online = connection.is_some(),
        "node ready (inference engine pending — Luminal GLM-5.2 pipeline)"
    );

    let stats = Arc::new(NodeStats::new());
    let mut ctx = NodeContext {
        identity,
        config,
        tps: tps_map,
        stats: Arc::clone(&stats),
    };

    let mut stats_interval = tokio::time::interval(Duration::from_secs(60));
    stats_interval.tick().await;
    loop {
        let event = tokio::select! {
            msg = recv_router_msg(&mut connection) => Event::RouterMsg(msg),
            _ = stats_interval.tick() => Event::StatsLog,
            _ = tokio::signal::ctrl_c() => Event::Shutdown,
        };

        match event {
            Event::RouterMsg(Ok(Some(msg))) => {
                handle_router_message(msg, &mut connection, &mut ctx);
            }
            Event::RouterMsg(Ok(None)) => {
                tracing::warn!("router stream closed, attempting reconnect");
                if let Some(ref conn) = connection {
                    conn.close();
                }
                connection = tokio::select! {
                    result = try_reconnect(&ctx) => result,
                    _ = tokio::signal::ctrl_c() => break,
                };
            }
            Event::RouterMsg(Err(e)) => {
                tracing::warn!(%e, "router communication error, attempting reconnect");
                if let Some(ref conn) = connection {
                    conn.close();
                }
                connection = tokio::select! {
                    result = try_reconnect(&ctx) => result,
                    _ = tokio::signal::ctrl_c() => break,
                };
            }
            Event::StatsLog => ctx.stats.log_summary(),
            Event::Shutdown => {
                tracing::info!("shutdown signal received");
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

enum Event {
    RouterMsg(Result<Option<RouterMessage>, error::NodeError>),
    StatsLog,
    Shutdown,
}

/// Receive a router message, or sleep 10s when offline (to allow periodic reconnect).
async fn recv_router_msg(
    connection: &mut Option<RouterConnection>,
) -> Result<Option<RouterMessage>, error::NodeError> {
    match connection {
        Some(ref mut conn) => conn.recv().await,
        None => {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Err(error::NodeError::Network("offline, attempting reconnect".into()))
        }
    }
}

/// Reconnect with exponential backoff (5 rounds, all URLs per round), then give up → offline.
async fn try_reconnect(ctx: &NodeContext) -> Option<RouterConnection> {
    let mut delay = Duration::from_secs(1);
    for round in 1..=5 {
        tracing::info!(round, delay_secs = delay.as_secs(), "attempting reconnect");
        tokio::time::sleep(delay).await;
        for url in &ctx.config.router_urls {
            match RouterConnection::connect(url, ctx.config.insecure, &ctx.identity, ctx.config.model_names.clone(), ctx.tps.clone(), placeholder_capacity()).await {
                Ok(conn) => {
                    tracing::info!(node_id = %conn.node_id, router = %url, "reconnected to router");
                    return Some(conn);
                }
                Err(e) => tracing::warn!(%e, router = %url, round, "reconnect attempt failed"),
            }
        }
        delay *= 2;
    }
    tracing::warn!("all reconnect attempts exhausted, running in offline mode");
    None
}

/// Handle a router message. Inference paths are stubbed until the Luminal GLM-5.2 engine lands;
/// the node still answers pings (status) and signs auth challenges.
fn handle_router_message(
    msg: RouterMessage,
    connection: &mut Option<RouterConnection>,
    ctx: &mut NodeContext,
) {
    match msg {
        RouterMessage::TaskAssignment { task_id, model, .. } => {
            tracing::warn!(%task_id, %model, "task assignment received but the Luminal GLM-5.2 engine is not wired yet — ignoring (node advertises zero capacity)");
        }
        RouterMessage::ValidationTask { validation_id, model, .. } => {
            tracing::warn!(%validation_id, %model, "validation task received but inference engine not wired — ignoring");
        }
        RouterMessage::ModelRegistryUpdate { entries } => {
            tracing::debug!(count = entries.len(), "model registry update ignored (model is predetermined GLM-5.2)");
        }
        RouterMessage::Ping => {
            if let Some(ref conn) = connection {
                let status = NodeMessage::StatusUpdate {
                    models: ctx.config.model_names.clone(),
                    capacity: placeholder_capacity(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    stats: Some(ctx.stats.snapshot()),
                };
                if let Err(e) = conn.send(status) {
                    tracing::error!(%e, "failed to send status update");
                }
            }
        }
        RouterMessage::Challenge { challenge } => {
            let (sig, recid) = ctx.identity.sign(&challenge);
            if let Some(ref conn) = connection {
                let response = NodeMessage::ChallengeResponse {
                    challenge,
                    signature: sig.serialize().to_vec(),
                    recovery_id: recid.serialize(),
                };
                if let Err(e) = conn.send(response) {
                    tracing::error!(%e, "failed to send challenge response");
                }
            }
        }
    }
}
