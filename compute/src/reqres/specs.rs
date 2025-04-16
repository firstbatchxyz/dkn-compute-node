use crate::DriaComputeNode;

use super::IsResponder;
use colored::Colorize;
use dkn_p2p::libp2p::{request_response::OutboundRequestId, PeerId};
use dkn_utils::{
    payloads::{Specs, SpecsRequest, SpecsResponse, SPECS_TOPIC},
    DriaMessage,
};
use eyre::{eyre, Result};
use uuid::Uuid;

pub struct SpecRequester;

impl IsResponder for SpecRequester {
    type Request = DriaMessage; // SpecRequest;
    type Response = SpecsResponse;
}

impl SpecRequester {
    pub(crate) async fn send_specs(
        node: &mut DriaComputeNode,
        peer_id: PeerId,
        specs: Specs,
    ) -> Result<OutboundRequestId> {
        let uuid = Uuid::new_v4();
        let specs_request = SpecsRequest { id: uuid, specs };

        let specs_message = node.new_message(
            serde_json::to_vec(&specs_request).expect("should be serializable"),
            SPECS_TOPIC,
        );
        let request_id = node.p2p.request(peer_id, specs_message).await?;

        // add it to local specs set
        node.specs_reqs.insert(uuid);

        Ok(request_id)
    }

    /// Handles the specs request received from the network.
    pub(crate) async fn handle_ack(node: &mut DriaComputeNode, res: SpecsResponse) -> Result<()> {
        if node.specs_reqs.remove(&res.id) {
            log::info!("{} request {} acknowledged.", SPECS_TOPIC.green(), res.id);
            Ok(())
        } else {
            Err(eyre!(
                "Received an unknown {} response with id {}.",
                SPECS_TOPIC.green(),
                res.id
            ))
        }
    }
}
