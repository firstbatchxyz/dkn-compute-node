use dkn_p2p::libp2p::{request_response::ResponseChannel, PeerId};
use eyre::Result;

use crate::responders::*;

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
}
