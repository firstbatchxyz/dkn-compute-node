use dkn_p2p::libp2p::{request_response::ResponseChannel, PeerId};
use eyre::{eyre, Result};

use crate::reqres::*;

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
        } else if let Ok(task_request) = WorkflowResponder::try_parse_request(&data) {
            log::info!("Received a task request from {}", peer_id);

            let workflow_message = WorkflowResponder::handle_compute(self, &task_request).await?;
            if let Err(e) = match workflow_message.batchable {
                // this is a batchable task, send it to batch worker
                // and keep track of the task id in pending tasks
                true => match self.workflow_batch_tx {
                    Some(ref mut tx) => {
                        self.pending_tasks_batch
                            .insert(workflow_message.task_id.clone(), channel);
                        tx.send(workflow_message).await
                    }
                    None => {
                        unreachable!("Batchable workflow received but no worker available.")
                    }
                },

                // this is a single task, send it to single worker
                // and keep track of the task id in pending tasks
                false => match self.workflow_single_tx {
                    Some(ref mut tx) => {
                        self.pending_tasks_single
                            .insert(workflow_message.task_id.clone(), channel);
                        tx.send(workflow_message).await
                    }
                    None => {
                        unreachable!("Single workflow received but no worker available.")
                    }
                },
            } {
                log::error!("Error sending workflow message: {:?}", e);
            };
        } else {
            return Err(eyre::eyre!(
                "Received unknown request from {}: {:?}",
                peer_id,
                data,
            ));
        };

        Ok(())
    }

    async fn handle_spec_request(
        &mut self,
        peer_id: PeerId,
        channel: ResponseChannel<Vec<u8>>,
        request: <SpecResponder as IsResponder>::Request,
    ) -> Result<()> {
        log::info!(
            "Got a spec request from peer {} with id {}",
            peer_id,
            request.request_id
        );

        let response = SpecResponder::respond(request, self.spec_collector.collect().await);
        let response_data = serde_json::to_vec(&response)?;

        log::info!(
            "Responding to spec request from peer {} with id {}",
            peer_id,
            response.request_id
        );
        self.p2p.respond(response_data, channel).await?;

        Ok(())
    }
}
