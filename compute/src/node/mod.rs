use dkn_p2p::{
    libp2p::{
        gossipsub::{Message, MessageId},
        request_response::ResponseChannel,
        PeerId,
    },
    DriaNodes, DriaP2PClient, DriaP2PCommander, DriaP2PProtocol,
};
use eyre::Result;
use std::collections::HashMap;
use tokio::{sync::mpsc, time::Instant};

use crate::{
    config::*,
    gossipsub::*,
    utils::{crypto::secret_to_keypair, refresh_dria_nodes, SpecCollector},
    workers::workflow::{WorkflowsWorker, WorkflowsWorkerInput, WorkflowsWorkerOutput},
};

mod core;
mod diagnostic;
mod gossipsub;
mod reqres;

/// Buffer size for message publishes.
const PUBLISH_CHANNEL_BUFSIZE: usize = 1024;

pub struct DriaComputeNode {
    pub config: DriaComputeNodeConfig,
    /// Pre-defined nodes that belong to Dria, e.g. bootstraps, relays and RPCs.
    pub dria_nodes: DriaNodes,
    /// Peer-to-peer client commander to interact with the network.
    pub p2p: DriaP2PCommander,
    /// The last time the node was pinged by the network.
    /// If this is too much, we can say that the node is not reachable by RPC.
    pub last_pinged_at: Instant,
    /// Gossipsub message receiver, used by peer-to-peer client in a separate thread.
    message_rx: mpsc::Receiver<(PeerId, MessageId, Message)>,
    /// Request-response request receiver.
    request_rx: mpsc::Receiver<(PeerId, Vec<u8>, ResponseChannel<Vec<u8>>)>,
    /// Publish receiver to receive messages to be published.
    publish_rx: mpsc::Receiver<WorkflowsWorkerOutput>,
    /// Workflow transmitter to send batchable tasks.
    workflow_batch_tx: Option<mpsc::Sender<WorkflowsWorkerInput>>,
    /// Workflow transmitter to send single tasks.
    workflow_single_tx: Option<mpsc::Sender<WorkflowsWorkerInput>>,
    // Single tasks
    pending_tasks_single: HashMap<String, ResponseChannel<Vec<u8>>>,
    // Batchable tasks
    pending_tasks_batch: HashMap<String, ResponseChannel<Vec<u8>>>,
    /// Completed single tasks count
    completed_tasks_single: usize,
    /// Completed batch tasks count
    completed_tasks_batch: usize,
    /// Spec collector for the node.
    spec_collector: SpecCollector,
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
        Option<WorkflowsWorker>,
        Option<WorkflowsWorker>,
    )> {
        // create the keypair from secret key
        let keypair = secret_to_keypair(&config.secret_key);

        // get available nodes (bootstrap, relay, rpc) for p2p
        let mut available_nodes = DriaNodes::new(config.network_type)
            .with_statics()
            .with_envs();
        if let Err(e) = refresh_dria_nodes(&mut available_nodes).await {
            log::error!("Error populating available nodes: {:?}", e);
        };

        // we are using the major.minor version as the P2P version
        // so that patch versions do not interfere with the protocol
        let protocol = DriaP2PProtocol::new_major_minor(config.network_type.protocol_name());
        log::info!("Using identity: {}", protocol);

        // create p2p client
        let (p2p_client, p2p_commander, message_rx, request_rx) = DriaP2PClient::new(
            keypair,
            config.p2p_listen_addr.clone(),
            &available_nodes,
            protocol,
        )?;

        // create workflow workers, all workers use the same publish channel
        let (publish_tx, publish_rx) = mpsc::channel(PUBLISH_CHANNEL_BUFSIZE);

        // check if we should create a worker for batchable workflows
        let (workflows_batch_worker, workflow_batch_tx) = if config.workflows.has_batchable_models()
        {
            let (worker, sender) = WorkflowsWorker::new(publish_tx.clone());
            (Some(worker), Some(sender))
        } else {
            (None, None)
        };

        // check if we should create a worker for single workflows
        let (workflows_single_worker, workflow_single_tx) =
            if config.workflows.has_non_batchable_models() {
                let (worker, sender) = WorkflowsWorker::new(publish_tx);
                (Some(worker), Some(sender))
            } else {
                (None, None)
            };

        let model_names = config.workflows.get_model_names();
        Ok((
            DriaComputeNode {
                config,
                p2p: p2p_commander,
                dria_nodes: available_nodes,
                publish_rx,
                message_rx,
                request_rx,
                workflow_batch_tx,
                workflow_single_tx,
                pending_tasks_single: HashMap::new(),
                pending_tasks_batch: HashMap::new(),
                completed_tasks_single: 0,
                completed_tasks_batch: 0,
                spec_collector: SpecCollector::new(model_names),
                last_pinged_at: Instant::now(),
            },
            p2p_client,
            workflows_batch_worker,
            workflows_single_worker,
        ))
    }
}
