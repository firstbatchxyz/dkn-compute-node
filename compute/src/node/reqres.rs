use dkn_p2p::libp2p::{request_response::ResponseChannel, PeerId};
use eyre::{eyre, Result};

use crate::{reqres::*, workers::task::TaskWorkerOutput};

use super::DriaComputeNode;

impl DriaComputeNode {
    /// Handles a request-response request received from the network.
    ///
    /// Internally, the data is expected to be some JSON serialized data that is expected to be parsed and handled.
    pub(crate) async fn handle_request(
        &mut self,
        (peer_id, data, channel): (PeerId, Vec<u8>, ResponseChannel<Vec<u8>>),
    ) -> Result<()> {
        // ensure that message is from the known RPCs
        if !self.dria_nodes.rpc_peerids.contains(&peer_id) {
            log::warn!("Received request from unauthorized source: {}", peer_id);
            log::debug!("Allowed sources: {:#?}", self.dria_nodes.rpc_peerids);
            return Err(eyre!("Received unauthorized request from {}", peer_id));
        }

        // try and parse the request
        if let Ok(spec_request) = SpecResponder::try_parse_request(&data) {
            self.handle_spec_request(peer_id, channel, spec_request)
                .await?;
        } else if let Ok(task_request) = TaskResponder::try_parse_request(&data) {
            self.handle_task_request(peer_id, channel, task_request)
                .await?;
        } else {
            return Err(eyre::eyre!(
                "Received unknown request from {}: {:?}",
                peer_id,
                data,
            ));
        };

        Ok(())
    }

    /// Handles a Specifications request received from the network.
    async fn handle_spec_request(
        &mut self,
        peer_id: PeerId,
        channel: ResponseChannel<Vec<u8>>,
        spec_request: <SpecResponder as IsResponder>::Request,
    ) -> Result<()> {
        log::info!(
            "Got a spec request from peer {} with id {}",
            peer_id,
            spec_request.request_id
        );

        let response = SpecResponder::respond(spec_request, self.spec_collector.collect().await);
        let response_data = serde_json::to_vec(&response)?;

        log::info!(
            "Responding to spec request from peer {} with id {}",
            peer_id,
            response.request_id
        );
        self.p2p.respond(response_data, channel).await?;

        Ok(())
    }

    /// Handles a Task request received from the network.
    ///
    /// Based on the task type, the task is sent to the appropriate worker & metadata is stored in memory.
    /// This metadata will be used during response as well, and we can count the number of tasks at hand by
    /// looking at the number metadata stored.
    async fn handle_task_request(
        &mut self,
        peer_id: PeerId,
        channel: ResponseChannel<Vec<u8>>,
        task_request: <TaskResponder as IsResponder>::Request,
    ) -> Result<()> {
        log::info!("Received a task request from {}", peer_id);

        let (task_input, task_metadata) =
            TaskResponder::prepare_worker_input(self, &task_request, channel).await?;
        if let Err(e) = match task_input.batchable {
            // this is a batchable task, send it to batch worker
            // and keep track of the task id in pending tasks
            true => match self.task_request_batch_tx {
                Some(ref mut tx) => {
                    self.pending_tasks_batch
                        .insert(task_input.task_id.clone(), task_metadata);
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
                        .insert(task_input.task_id.clone(), task_metadata);
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

    pub(crate) async fn handle_task_response(
        &mut self,
        task_response: TaskWorkerOutput,
    ) -> Result<()> {
        // remove the task from pending tasks, and get its metadata
        let task_metadata = match task_response.batchable {
            true => {
                self.completed_tasks_batch += 1; // TODO: this should be done in success
                self.pending_tasks_batch.remove(&task_response.task_id)
            }
            false => {
                self.completed_tasks_single += 1; // TODO: this should be done in success
                self.pending_tasks_single.remove(&task_response.task_id)
            }
        };

        // respond to the response channel with the result
        match task_metadata {
            Some(channel) => {
                TaskResponder::handle_respond(self, task_response, channel).await?;
            }
            None => {
                return Err(eyre!(
                    "Channel not found for task id: {}",
                    task_response.task_id
                ))
            }
        };

        Ok(())
    }
}
