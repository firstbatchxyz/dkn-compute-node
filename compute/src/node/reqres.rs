use colored::Colorize;
use dkn_p2p::libp2p::{
    request_response::{OutboundRequestId, ResponseChannel},
    PeerId,
};
use dkn_p2p::DriaReqResMessage;
use dkn_utils::payloads::{HEARTBEAT_TOPIC, SPECS_TOPIC, TASK_REQUEST_TOPIC};
use eyre::{eyre, Result};

use crate::{reqres::*, workers::task::TaskWorkerOutput};

use super::DriaComputeNode;

impl DriaComputeNode {
    /// Handles a generic request-response message received from the network.
    ///
    /// - Request is forwarded to [`handle_request`](DriaComputeNode::handle_request) method.
    /// - Response is forwarded to [`handle_response`](DriaComputeNode::handle_response) method.
    ///
    /// Does not return an error, but simply logs it to [`log::error`].
    pub(crate) async fn handle_reqres(&mut self, peer_id: PeerId, message: DriaReqResMessage) {
        match message {
            // make sure that the `channel` here is NOT DROPPED until a response is sent,
            // otherwise you will get an error
            DriaReqResMessage::Request {
                request,
                request_id,
                channel,
            } => {
                log::debug!("Received a request ({}) from {}", request_id, peer_id);

                // ensure that message is from the known RPCs
                if self.dria_rpc.peer_id != peer_id {
                    log::warn!("Received request from unauthorized source: {}", peer_id);
                    log::debug!("Allowed source: {}", self.dria_rpc.peer_id);
                } else if let Err(e) = self.handle_request(peer_id, request, channel).await {
                    log::error!("Error handling request: {:?}", e);
                }
            }

            DriaReqResMessage::Response {
                response,
                request_id,
            } => {
                log::debug!("Received a response ({}) from {}", request_id, peer_id);
                if let Err(e) = self.handle_response(peer_id, request_id, response).await {
                    log::error!("Error handling response: {:?}", e);
                }
            }
        };
    }

    /// Handles a [`request_response`] response received from the network.
    ///
    /// - Internally, the data is expected to be some JSON serialized data that is expected to be parsed and handled.
    /// - Can be inlined because it is only called by [`DriaComputeNode::handle_reqres`].
    #[inline]
    async fn handle_response(
        &mut self,
        peer_id: PeerId,
        request_id: OutboundRequestId,
        data: Vec<u8>,
    ) -> Result<()> {
        if let Ok(heartbeat_response) = HeartbeatRequester::try_parse_response(&data) {
            log::info!(
                "Received a {} response ({request_id}) from {peer_id}",
                HEARTBEAT_TOPIC.blue(),
            );
            HeartbeatRequester::handle_ack(self, heartbeat_response).await
        } else if let Ok(spec_response) = SpecRequester::try_parse_response(&data) {
            log::info!(
                "Received a {} response ({request_id}) from {peer_id}",
                SPECS_TOPIC.green(),
            );
            SpecRequester::handle_ack(self, spec_response).await
        } else {
            Err(eyre::eyre!("Received unhandled request from {}", peer_id))
        }
    }

    /// Handles a [`request_response`] request received from the network.
    ///
    /// - Internally, the data is expected to be some JSON serialized data that is expected to be parsed and handled.
    /// - Can be inlined because it is only called by [`DriaComputeNode::handle_reqres`].
    async fn handle_request(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<Vec<u8>>,
    ) -> Result<()> {
        if let Ok(task_request) = TaskResponder::try_parse_request(&data) {
            self.handle_task_request(peer_id, task_request, channel)
                .await
        } else {
            Err(eyre::eyre!("Received unhandled request from {peer_id}"))
        }
    }

    /// Handles a Task request received from the network.
    ///
    /// Based on the task type, the task is sent to the appropriate worker & metadata is stored in memory.
    /// This metadata will be used during response as well, and we can count the number of tasks at hand by
    /// looking at the number metadata stored.
    async fn handle_task_request(
        &mut self,
        peer_id: PeerId,
        task_request: <TaskResponder as IsResponder>::Request,
        channel: ResponseChannel<Vec<u8>>,
    ) -> Result<()> {
        log::info!(
            "Received a {} request from {peer_id}",
            TASK_REQUEST_TOPIC.yellow()
        );

        let (task_input, task_metadata) =
            TaskResponder::prepare_worker_input(self, &task_request, channel).await?;
        if let Err(e) = match task_input.task.is_batchable() {
            // this is a batchable task, send it to batch worker
            // and keep track of the task id in pending tasks
            true => match self.task_request_batch_tx {
                Some(ref mut tx) => {
                    self.pending_tasks_batch
                        .insert(task_input.row_id, task_metadata);
                    tx.send(task_input).await
                }
                None => {
                    return Err(eyre!(
                        "Batchable workflow received but no worker available."
                    ));
                }
            },

            // this is a single task, send it to single worker
            // and keep track of the task id in pending tasks
            false => match self.task_request_single_tx {
                Some(ref mut tx) => {
                    self.pending_tasks_single
                        .insert(task_input.row_id, task_metadata);
                    tx.send(task_input).await
                }
                None => {
                    return Err(eyre!("Single workflow received but no worker available."));
                }
            },
        } {
            log::error!("Error sending workflow message: {:?}", e);
        };

        Ok(())
    }

    pub(crate) async fn send_task_output(&mut self, task_response: TaskWorkerOutput) -> Result<()> {
        // remove the task from pending tasks, and get its metadata
        let task_metadata = match task_response.batchable {
            true => {
                self.completed_tasks_batch += 1; // TODO: this should be done in success
                self.pending_tasks_batch.remove(&task_response.row_id)
            }
            false => {
                self.completed_tasks_single += 1; // TODO: this should be done in success
                self.pending_tasks_single.remove(&task_response.row_id)
            }
        };

        // respond to the response channel with the result
        match task_metadata {
            Some(task_metadata) => {
                TaskResponder::send_output(self, task_response, task_metadata).await?;
            }
            None => {
                // totally unexpected case, wont happen at all
                eyre::bail!("Metadata not found for {}", task_response.row_id);
            }
        };

        Ok(())
    }

    /// Sends a heartbeat request to the configured RPC node.
    #[inline]
    pub(crate) async fn send_heartbeat(&mut self) -> Result<()> {
        let peer_id = self.dria_rpc.peer_id;
        let request_id = HeartbeatRequester::send_heartbeat(self, peer_id).await?;
        log::info!(
            "Sending {} request ({request_id}) to {peer_id}",
            HEARTBEAT_TOPIC.blue()
        );

        Ok(())
    }

    /// Sends a specs request to the configured RPC node.
    #[inline]
    pub(crate) async fn send_specs(&mut self) -> Result<()> {
        let peer_id = self.dria_rpc.peer_id;
        let specs = self.spec_collector.collect().await;
        let request_id = SpecRequester::send_specs(self, peer_id, specs).await?;
        log::info!(
            "Sending {} request ({request_id}) to {peer_id}",
            SPECS_TOPIC.green()
        );

        Ok(())
    }
}
