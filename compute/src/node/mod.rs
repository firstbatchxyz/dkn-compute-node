use dkn_p2p::{
    libp2p::PeerId, DriaP2PClient, DriaP2PCommander, DriaP2PProtocol, DriaReqResMessage,
};
use eyre::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::{
    config::*,
    utils::{crypto::secret_to_keypair, get_points, DriaRPC, SpecCollector},
    workers::task::{TaskWorker, TaskWorkerInput, TaskWorkerMetadata, TaskWorkerOutput},
};

mod core;
mod diagnostic;
mod reqres;

/// Buffer size for message publishes.
const PUBLISH_CHANNEL_BUFSIZE: usize = 1024;

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    /// Chosen RPC node.
    pub dria_rpc: DriaRPC,
    /// Peer-to-peer client commander to interact with the network.
    pub p2p: DriaP2PCommander,
    /// The last time the node had an acknowledged heartbeat.
    /// If this is too much, we can say that the node is not reachable by RPC.
    pub(crate) last_heartbeat_at: chrono::DateTime<chrono::Utc>,
    /// Number of pings received.
    pub(crate) num_heartbeats: u64,
    /// A mapping of heartbeat UUIDs to their deadlines.
    /// This is used to track the heartbeats, and their acknowledgements.
    pub(crate) heartbeats: HashMap<uuid::Uuid, chrono::DateTime<chrono::Utc>>,
    /// Request-response message receiver, can have both a request or a response.
    reqres_rx: mpsc::Receiver<(PeerId, DriaReqResMessage)>,
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

        // get available rpc node
        let dria_nodes = DriaRPC::new(config.network_type)
            .await
            .expect("could not get RPC to connect to");

        // we are using the major.minor version as the P2P version
        // so that patch versions do not interfere with the protocol
        let protocol = DriaP2PProtocol::new_major_minor(config.network_type.protocol_name());
        log::info!("Using identity: {}", protocol);

        // create p2p client
        let (p2p_client, p2p_commander, request_rx) = DriaP2PClient::new(
            keypair,
            config.p2p_listen_addr.clone(),
            &dria_nodes.addr,
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

        let initial_steps = get_points(&config.address)
            .await
            .map(|s| s.score)
            .unwrap_or_default();

        Ok((
            DriaComputeNode {
                config,
                p2p: p2p_commander,
                dria_rpc: dria_nodes,
                // receivers
                task_output_rx: publish_rx,
                reqres_rx: request_rx,
                // transmitters
                task_request_batch_tx: task_batch_tx,
                task_request_single_tx: task_single_tx,
                // task trackers
                pending_tasks_single: HashMap::new(),
                pending_tasks_batch: HashMap::new(),
                completed_tasks_single: 0,
                completed_tasks_batch: 0,
                // heartbeats
                heartbeats: HashMap::new(),
                last_heartbeat_at: chrono::Utc::now(),
                num_heartbeats: 0,
                // misc
                initial_steps,
                spec_collector: SpecCollector::new(model_names),
            },
            p2p_client,
            task_batch_worker,
            task_single_worker,
        ))
    }
}
