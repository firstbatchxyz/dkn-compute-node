use eyre::Result;
use libp2p::identity::{Keypair, PublicKey};
use libp2p::{identify, request_response, StreamProtocol};
use std::time::Duration;

#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct DriaBehaviour {
    pub identify: identify::Behaviour,
    pub request_response: request_response::cbor::Behaviour<Vec<u8>, Vec<u8>>,
}

impl DriaBehaviour {
    pub fn new(key: &Keypair, identity_protocol: String, reqres_protocol: StreamProtocol) -> Self {
        let public_key = key.public();

        Self {
            identify: create_identify_behaviour(public_key, identity_protocol),
            request_response: create_request_response_behaviour(reqres_protocol),
        }
    }
}

/// Configures the request-response behaviour for the node.
///
/// The protocol supports bytes only,
#[inline]
fn create_request_response_behaviour(
    protocol_name: StreamProtocol,
) -> request_response::cbor::Behaviour<Vec<u8>, Vec<u8>> {
    use request_response::{Behaviour, Config, ProtocolSupport};

    const REQUEST_RESPONSE_TIMEOUT: Duration = Duration::from_secs(512);

    Behaviour::new(
        [(protocol_name, ProtocolSupport::Full)],
        Config::default().with_request_timeout(REQUEST_RESPONSE_TIMEOUT),
    )
}

/// Configures the Identify behavior to allow nodes to exchange information like supported protocols.
#[inline]
fn create_identify_behaviour(
    local_public_key: PublicKey,
    protocol_version: String,
) -> identify::Behaviour {
    use identify::{Behaviour, Config};

    Behaviour::new(Config::new(protocol_version, local_public_key))
}
