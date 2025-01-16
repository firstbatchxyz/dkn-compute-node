use dkn_p2p::{
    libp2p::{
        gossipsub::{Message, MessageAcceptance, MessageId},
        request_response::ResponseChannel,
        PeerId,
    },
    DriaNodes, DriaP2PClient, DriaP2PCommander, DriaP2PProtocol,
};
use eyre::Result;
use std::collections::HashSet;
use tokio::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio_util::{either::Either, sync::CancellationToken};

use crate::{
    config::*,
    handlers::*,
    responders::{IsResponder, SpecResponder, WorkflowResponder},
    utils::{crypto::secret_to_keypair, refresh_dria_nodes, DriaMessage, SpecCollector},
    workers::workflow::{WorkflowsWorker, WorkflowsWorkerInput, WorkflowsWorkerOutput},
    DRIA_COMPUTE_NODE_VERSION,
};

/// Number of seconds between refreshing for diagnostic prints.
const DIAGNOSTIC_REFRESH_INTERVAL_SECS: u64 = 30;
/// Number of seconds between refreshing the available nodes.
const AVAILABLE_NODES_REFRESH_INTERVAL_SECS: u64 = 30 * 60; // 30 minutes
/// Number of seconds such that if the last ping is older than this, the node is considered unreachable.
const PING_LIVENESS_SECS: u64 = 150;
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
    /// Publish receiver to receive messages to be published,
    publish_rx: mpsc::Receiver<WorkflowsWorkerOutput>,
    /// Workflow transmitter to send batchable tasks.
    workflow_batch_tx: Option<mpsc::Sender<WorkflowsWorkerInput>>,
    /// Workflow transmitter to send single tasks.
    workflow_single_tx: Option<mpsc::Sender<WorkflowsWorkerInput>>,
    // Single tasks hash-map
    pending_tasks_single: HashSet<String>,
    // Batch tasks hash-map
    pending_tasks_batch: HashSet<String>,
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
            let worker = WorkflowsWorker::new(publish_tx.clone());
            (Some(worker.0), Some(worker.1))
        } else {
            (None, None)
        };

        // check if we should create a worker for single workflows
        let (workflows_single_worker, workflow_single_tx) =
            if config.workflows.has_non_batchable_models() {
                let worker = WorkflowsWorker::new(publish_tx);
                (Some(worker.0), Some(worker.1))
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
                pending_tasks_single: HashSet::new(),
                pending_tasks_batch: HashSet::new(),
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

    /// Subscribe to a certain task with its topic.
    ///
    /// These are likely to be called once, so can be inlined.
    #[inline]
    pub async fn subscribe(&mut self, topic: &str) -> Result<()> {
        let ok = self.p2p.subscribe(topic).await?;
        if ok {
            log::info!("Subscribed to {}", topic);
        } else {
            log::info!("Already subscribed to {}", topic);
        }
        Ok(())
    }

    /// Unsubscribe from a certain task with its topic.
    ///
    /// These are likely to be called once, so can be inlined.
    #[inline]
    pub async fn unsubscribe(&mut self, topic: &str) -> Result<()> {
        let ok = self.p2p.unsubscribe(topic).await?;
        if ok {
            log::info!("Unsubscribed from {}", topic);
        } else {
            log::info!("Already unsubscribed from {}", topic);
        }
        Ok(())
    }

    /// Returns the task count within the channels, `single` and `batch`.
    #[inline]
    pub fn get_pending_task_count(&self) -> [usize; 2] {
        [
            self.pending_tasks_single.len(),
            self.pending_tasks_batch.len(),
        ]
    }

    /// Publishes a given message to the network w.r.t the topic of it.
    ///
    /// Internally, identity is attached to the the message which is then JSON serialized to bytes
    /// and then published to the network as is.
    pub async fn publish(&mut self, mut message: DriaMessage) -> Result<()> {
        // attach protocol name to the message
        message = message.with_identity(self.p2p.protocol().name.clone());

        let message_bytes = serde_json::to_vec(&message)?;
        let message_id = self.p2p.publish(&message.topic, message_bytes).await?;
        log::info!("Published message ({}) to {}", message_id, message.topic);
        Ok(())
    }

    /// Returns the list of connected peers, `mesh` and `all`.
    #[inline(always)]
    pub async fn peers(&self) -> Result<(Vec<PeerId>, Vec<PeerId>)> {
        self.p2p.peers().await
    }

    /// Handles a GossipSub message received from the network.
    async fn handle_message(
        &mut self,
        (peer_id, message_id, gossipsub_message): (PeerId, &MessageId, Message),
    ) -> MessageAcceptance {
        // handle message with respect to its topic
        match gossipsub_message.topic.as_str() {
            PingpongHandler::LISTEN_TOPIC | WorkflowHandler::LISTEN_TOPIC => {
                // ensure that the message is from a valid source (origin)
                let Some(source_peer_id) = gossipsub_message.source else {
                    log::warn!(
                        "Received {} message from {} without source.",
                        gossipsub_message.topic,
                        peer_id
                    );
                    return MessageAcceptance::Ignore;
                };

                // ensure that message is from the known RPCs
                if !self.dria_nodes.rpc_peerids.contains(&source_peer_id) {
                    log::warn!(
                        "Received message from unauthorized source: {}",
                        source_peer_id
                    );
                    log::debug!("Allowed sources: {:#?}", self.dria_nodes.rpc_peerids);
                    return MessageAcceptance::Ignore;
                }

                // parse the raw gossipsub message to a prepared DKN message
                // the received message is expected to use IdentHash for the topic, so we can see the name of the topic immediately.
                log::debug!("Parsing {} message.", gossipsub_message.topic.as_str());
                let message: DriaMessage = match serde_json::from_slice(&gossipsub_message.data) {
                    Ok(message) => message,
                    Err(e) => {
                        log::error!("Error parsing message: {:?}", e);
                        log::debug!(
                            "Message: {}",
                            String::from_utf8_lossy(&gossipsub_message.data)
                        );
                        return MessageAcceptance::Ignore;
                    }
                };

                // debug-log the received message
                log::debug!(
                    "Received {} message ({}) from {}\n{}",
                    gossipsub_message.topic,
                    message_id,
                    peer_id,
                    message
                );

                // check signature
                match message.is_signed(&self.config.admin_public_key) {
                    Ok(true) => { /* message is signed correctly, nothing to do here */ }
                    Ok(false) => {
                        log::warn!("Message has wrong signature!");
                        return MessageAcceptance::Reject;
                    }
                    Err(e) => {
                        log::error!("Error verifying signature: {:?}", e);
                        return MessageAcceptance::Ignore;
                    }
                }

                // handle the DKN message with respect to the topic
                let handler_result = match message.topic.as_str() {
                    WorkflowHandler::LISTEN_TOPIC => {
                        match WorkflowHandler::handle_compute(self, &message).await {
                            // we got acceptance, so something was not right about the workflow and we can ignore it
                            Ok(Either::Left(acceptance)) => Ok(acceptance),
                            // we got the parsed workflow itself, send to a worker thread w.r.t batchable
                            Ok(Either::Right(workflow_message)) => {
                                if let Err(e) = match workflow_message.batchable {
                                    // this is a batchable task, send it to batch worker
                                    // and keep track of the task id in pending tasks
                                    true => match self.workflow_batch_tx {
                                        Some(ref mut tx) => {
                                            self.pending_tasks_batch
                                                .insert(workflow_message.task_id.clone());
                                            tx.send(workflow_message).await
                                        }
                                        None => unreachable!(
                                            "Batchable workflow received but no worker available."
                                        ),
                                    },
                                    // this is a single task, send it to single worker
                                    // and keep track of the task id in pending tasks
                                    false => match self.workflow_single_tx {
                                        Some(ref mut tx) => {
                                            self.pending_tasks_single
                                                .insert(workflow_message.task_id.clone());
                                            tx.send(workflow_message).await
                                        }
                                        None => unreachable!(
                                            "Single workflow received but no worker available."
                                        ),
                                    },
                                } {
                                    log::error!("Error sending workflow message: {:?}", e);
                                };

                                // accept the message in case others may be included in the filter as well
                                Ok(MessageAcceptance::Accept)
                            }
                            // something went wrong, handle this outside
                            Err(err) => Err(err),
                        }
                    }
                    PingpongHandler::LISTEN_TOPIC => {
                        PingpongHandler::handle_ping(self, &message).await
                    }
                    _ => unreachable!("unreachable due to match expression"),
                };

                // validate the message based on the result
                handler_result.unwrap_or_else(|err| {
                    log::error!("Error handling {} message: {:?}", message.topic, err);
                    MessageAcceptance::Ignore
                })
            }
            PingpongHandler::RESPONSE_TOPIC | WorkflowHandler::RESPONSE_TOPIC => {
                // since we are responding to these topics, we might receive messages from other compute nodes
                // we can gracefully ignore them and propagate it to to others
                log::trace!("Ignoring {} message", gossipsub_message.topic);
                MessageAcceptance::Accept
            }
            other => {
                // reject this message as its from a foreign topic
                log::warn!("Received message from unexpected topic: {}", other);
                MessageAcceptance::Reject
            }
        }
    }

    /// Handles a request-response request received from the network.
    ///
    /// Internally, the data is expected to be some JSON serialized data that is expected to be parsed and handled.
    async fn handle_request(
        &mut self,
        (peer_id, data, channel): (PeerId, Vec<u8>, ResponseChannel<Vec<u8>>),
    ) -> Result<()> {
        // ensure that message is from the known RPCs
        if !self.dria_nodes.rpc_peerids.contains(&peer_id) {
            log::warn!("Received request from unauthorized source: {}", peer_id);
            log::debug!("Allowed sources: {:#?}", self.dria_nodes.rpc_peerids);
            return Err(eyre::eyre!(
                "Received unauthorized request from {}",
                peer_id
            ));
        }

        // respond w.r.t data
        let response_data = if let Ok(req) = SpecResponder::try_parse_request(&data) {
            log::info!(
                "Got a spec request from peer {} with id {}",
                peer_id,
                req.request_id
            );

            let response = SpecResponder::respond(req, self.spec_collector.collect().await);
            serde_json::to_vec(&response)?
        } else if let Ok(req) = WorkflowResponder::try_parse_request(&data) {
            log::info!("Received a task request with id: {}", req.task_id);
            return Err(eyre::eyre!(
                "REQUEST RESPONSE FOR TASKS ARE NOT IMPLEMENTED YET"
            ));
        } else {
            return Err(eyre::eyre!(
                "Received unknown request from {}: {:?}",
                peer_id,
                data,
            ));
        };

        log::info!("Responding to peer {}", peer_id);
        self.p2p.respond(response_data, channel).await
    }

    /// Runs the main loop of the compute node.
    /// This method is not expected to return until cancellation occurs for the given token.
    pub async fn run(&mut self, cancellation: CancellationToken) -> Result<()> {
        // prepare durations for sleeps
        let mut diagnostic_refresh_interval =
            tokio::time::interval(Duration::from_secs(DIAGNOSTIC_REFRESH_INTERVAL_SECS));
        diagnostic_refresh_interval.tick().await; // move one tick
        let mut available_node_refresh_interval =
            tokio::time::interval(Duration::from_secs(AVAILABLE_NODES_REFRESH_INTERVAL_SECS));
        available_node_refresh_interval.tick().await; // move one tick

        // subscribe to topics
        self.subscribe(PingpongHandler::LISTEN_TOPIC).await?;
        self.subscribe(PingpongHandler::RESPONSE_TOPIC).await?;
        self.subscribe(WorkflowHandler::LISTEN_TOPIC).await?;
        self.subscribe(WorkflowHandler::RESPONSE_TOPIC).await?;

        loop {
            tokio::select! {

                // a Workflow message to be published is received from the channel
                // this is expected to be sent by the workflow worker
                publish_msg_opt = self.publish_rx.recv() => {
                    if let Some(publish_msg) = publish_msg_opt {
                        // remove the task from pending tasks based on its batchability
                        match publish_msg.batchable {
                            true => {
                                self.completed_tasks_batch += 1;
                                self.pending_tasks_batch.remove(&publish_msg.task_id);
                            },
                            false => {
                                self.completed_tasks_single += 1;
                                self.pending_tasks_single.remove(&publish_msg.task_id);
                            }
                        };

                        // publish the message
                        WorkflowHandler::handle_publish(self, publish_msg).await?;
                    } else {
                        log::error!("Publish channel closed unexpectedly.");
                        break;
                    };
                },

                // check peer count every now and then
                _ = diagnostic_refresh_interval.tick() => self.handle_diagnostic_refresh().await,
                // available nodes are refreshed every now and then
                _ = available_node_refresh_interval.tick() => self.handle_available_nodes_refresh().await,
                // a GossipSub message is received from the channel
                // this is expected to be sent by the p2p client
                gossipsub_msg_opt = self.message_rx.recv() => {
                    if let Some((peer_id, message_id, message)) = gossipsub_msg_opt {
                        // handle the message, returning a message acceptance for the received one
                        let acceptance = self.handle_message((peer_id, &message_id, message)).await;

                        // validate the message based on the acceptance
                        // cant do anything but log if this gives an error as well
                        if let Err(e) = self.p2p.validate_message(&message_id, &peer_id, acceptance).await {
                            log::error!("Error validating message {}: {:?}", message_id, e);
                        }
                    } else {
                        log::error!("message_rx channel closed unexpectedly.");
                        break;
                    };
                },
                // a Response message is received from the channel
                // this is expected to be sent by the p2p client
                request_msg_opt = self.request_rx.recv() => {
                    if let Some((peer_id, data, channel)) = request_msg_opt {
                        if let Err(e) = self.handle_request((peer_id, data, channel)).await {
                            log::error!("Error handling request: {:?}", e);
                        }
                    } else {
                        log::error!("request_rx channel closed unexpectedly.");
                        break;
                    };
                },
                // check if the cancellation token is cancelled
                // this is expected to be cancelled by the main thread with signal handling
                _ = cancellation.cancelled() => break,
            }
        }

        // unsubscribe from topics
        self.unsubscribe(PingpongHandler::LISTEN_TOPIC).await?;
        self.unsubscribe(PingpongHandler::RESPONSE_TOPIC).await?;
        self.unsubscribe(WorkflowHandler::LISTEN_TOPIC).await?;
        self.unsubscribe(WorkflowHandler::RESPONSE_TOPIC).await?;

        // print one final diagnostic as a summary
        self.handle_diagnostic_refresh().await;

        // shutdown channels
        self.shutdown().await?;

        Ok(())
    }

    /// Shutdown channels between p2p, worker and yourself.
    pub async fn shutdown(&mut self) -> Result<()> {
        log::debug!("Sending shutdown command to p2p client.");
        self.p2p.shutdown().await?;

        log::debug!("Closing message channel.");
        self.message_rx.close();

        log::debug!("Closing publish channel.");
        self.publish_rx.close();

        Ok(())
    }

    /// Peer refresh simply reports the peer count to the user.
    async fn handle_diagnostic_refresh(&self) {
        let mut diagnostics = Vec::new();

        // print peer counts
        match self.p2p.peer_counts().await {
            Ok((mesh, all)) => {
                diagnostics.push(format!("Peer Count (mesh/all): {} / {}", mesh, all))
            }
            Err(e) => log::error!("Error getting peer counts: {:?}", e),
        }

        // print tasks count
        let [single, batch] = self.get_pending_task_count();
        diagnostics.push(format!(
            "Pending Tasks (single/batch): {} / {}",
            single, batch
        ));

        // completed tasks count is printed as well in debug
        if log::log_enabled!(log::Level::Debug) {
            diagnostics.push(format!(
                "Completed Tasks (single/batch): {} / {}",
                self.completed_tasks_single, self.completed_tasks_batch
            ));
        }

        // print version
        diagnostics.push(format!("Version: v{}", DRIA_COMPUTE_NODE_VERSION));

        log::info!("{}", diagnostics.join(" | "));

        if self.last_pinged_at < Instant::now() - Duration::from_secs(PING_LIVENESS_SECS) {
            log::error!(
                "Node has not received any pings for at least {} seconds & it may be unreachable!\nPlease restart your node!",
                PING_LIVENESS_SECS
            );
        }
    }

    /// Updates the local list of available nodes by refreshing it.
    /// Dials the RPC nodes again for better connectivity.
    async fn handle_available_nodes_refresh(&mut self) {
        log::info!("Refreshing available Dria nodes.");

        // refresh available nodes
        if let Err(e) = refresh_dria_nodes(&mut self.dria_nodes).await {
            log::error!("Error refreshing available nodes: {:?}", e);
        };

        // dial all rpc nodes
        for rpc_addr in self.dria_nodes.rpc_nodes.iter() {
            log::info!("Dialling RPC node: {}", rpc_addr);
            if let Err(e) = self.p2p.dial(rpc_addr.clone()).await {
                log::warn!("Error dialling RPC node: {:?}", e);
            };
        }

        log::info!("Finished refreshing!");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_publish_message() -> eyre::Result<()> {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("dkn_compute", log::LevelFilter::Debug)
            .filter_module("dkn_p2p", log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // create node
        let cancellation = CancellationToken::new();
        let (mut node, p2p, _, _) = DriaComputeNode::new(DriaComputeNodeConfig::default()).await?;

        // spawn p2p task
        let p2p_task = tokio::spawn(async move { p2p.run().await });

        // launch & wait for a while for connections
        log::info!("Waiting a bit for peer setup.");
        let run_cancellation = cancellation.clone();
        tokio::select! {
            _ = node.run(run_cancellation) => (),
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(20)) => cancellation.cancel(),
        }
        log::info!("Connected Peers:\n{:#?}", node.peers().await?);

        // publish a dummy message
        let topic = "foo";
        let message = DriaMessage::new("hello from the other side", topic);
        node.subscribe(topic).await?;
        node.publish(message).await?;
        node.unsubscribe(topic).await?;

        // close everything
        log::info!("Shutting down node.");
        node.p2p.shutdown().await?;

        // wait for task handle
        p2p_task.await?;

        Ok(())
    }
}
