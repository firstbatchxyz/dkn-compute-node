use dkn_p2p::{
    libp2p::{request_response::ResponseChannel, PeerId},
    DriaNodes, DriaP2PClient, DriaP2PCommander, DriaP2PProtocol,
};
use eyre::Result;
use std::collections::HashMap;
use tokio::{sync::mpsc, time::Instant};

use crate::{
    config::*,
    utils::{crypto::secret_to_keypair, get_steps, refresh_dria_nodes, SpecCollector},
    workers::task::{TaskWorker, TaskWorkerInput, TaskWorkerMetadata, TaskWorkerOutput},
};

mod core;
mod diagnostic;
mod reqres;

/// Buffer size for message publishes.
const PUBLISH_CHANNEL_BUFSIZE: usize = 1024;

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    /// Pre-defined nodes that belong to Dria, e.g. bootstraps, relays and RPCs.
    pub dria_nodes: DriaNodes,
    /// Peer-to-peer client commander to interact with the network.
    pub p2p: DriaP2PCommander,
    /// The last time the node had an acknowledged heartbeat.
    /// If this is too much, we can say that the node is not reachable by RPC.
    pub(crate) last_heartbeat_at: Instant,
    /// Number of pings received.
    pub(crate) num_heartbeats: u64,
    /// The time the node was started.
    pub(crate) started_at: Instant,
    /// Request-response request receiver.
    request_rx: mpsc::Receiver<(PeerId, Vec<u8>, ResponseChannel<Vec<u8>>)>,
    /// Task response receiver, will respond to the request-response channel with the given result.
    task_output_rx: mpsc::Receiver<TaskWorkerOutput>,
    /// Task worker transmitter to send batchable tasks.
    task_request_batch_tx: Option<mpsc::Sender<TaskWorkerInput>>,
    /// Task worker transmitter to send single tasks.
    task_request_single_tx: Option<mpsc::Sender<TaskWorkerInput>>,
    // Single tasks
    pending_tasks_single: HashMap<String, TaskWorkerMetadata>,
    // Batchable tasks
    pending_tasks_batch: HashMap<String, TaskWorkerMetadata>,
    /// Completed single tasks count
    completed_tasks_single: usize,
    /// Completed batch tasks count
    completed_tasks_batch: usize,
    /// Specifications collector.
    spec_collector: SpecCollector,
    /// Initial steps count.
    initial_steps: u64,
}

impl DriaComputeNode {
    /// Creates a new `DriaComputeNode` with the given configuration and cancellation token.
    ///
    /// Returns the node instance and p2p client together. P2p MUST be run in a separate task before this node is used at all.
    pub async fn new(
        config: DriaComputeNodeConfig,
    ) -> Result<(
        DriaComputeNode,
        DriaP2PClient,
        Option<TaskWorker>,
        Option<TaskWorker>,
    )> {
        // create the keypair from secret key
        let keypair = secret_to_keypair(&config.secret_key);

        // get available nodes (bootstrap, relay, rpc) for p2p
        let mut dria_nodes = DriaNodes::new(config.network_type).with_statics();
        if let Err(e) = refresh_dria_nodes(&mut dria_nodes).await {
            log::error!("Error populating available nodes: {:?}", e);
        };

        // we are using the major.minor version as the P2P version
        // so that patch versions do not interfere with the protocol
        let protocol = DriaP2PProtocol::new_major_minor(config.network_type.protocol_name());
        log::info!("Using identity: {}", protocol);

        // create p2p client
        let (p2p_client, p2p_commander, request_rx) = DriaP2PClient::new(
            keypair,
            config.p2p_listen_addr.clone(),
            &dria_nodes,
            protocol,
        )?;

        // create workflow workers, all workers use the same publish channel
        let (publish_tx, publish_rx) = mpsc::channel(PUBLISH_CHANNEL_BUFSIZE);

        // check if we should create a worker for batchable workflows
        let (task_batch_worker, task_batch_tx) = if config.workflows.has_batchable_models() {
            let (worker, sender) = TaskWorker::new(publish_tx.clone());
            (Some(worker), Some(sender))
        } else {
            (None, None)
        };

        // check if we should create a worker for single workflows
        let (task_single_worker, task_single_tx) = if config.workflows.has_non_batchable_models() {
            let (worker, sender) = TaskWorker::new(publish_tx);
            (Some(worker), Some(sender))
        } else {
            (None, None)
        };

        let model_names = config.workflows.get_model_names();

        let initial_steps = get_steps(&config.address)
            .await
            .map(|s| s.score)
            .unwrap_or_default();

        Ok((
            DriaComputeNode {
                config,
                p2p: p2p_commander,
                dria_nodes,
                // receivers
                task_output_rx: publish_rx,
                request_rx,
                // transmitters
                task_request_batch_tx: task_batch_tx,
                task_request_single_tx: task_single_tx,
                // task trackers
                pending_tasks_single: HashMap::new(),
                pending_tasks_batch: HashMap::new(),
                completed_tasks_single: 0,
                completed_tasks_batch: 0,
                // others
                initial_steps,
                spec_collector: SpecCollector::new(model_names),
                last_heartbeat_at: Instant::now(),
                num_heartbeats: 0,
                started_at: Instant::now(),
            },
            p2p_client,
            task_batch_worker,
            task_single_worker,
        ))
    }
}
