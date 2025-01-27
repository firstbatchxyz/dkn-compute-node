use base64::{prelude::BASE64_STANDARD, Engine};
use core::fmt;
use dkn_p2p::libp2p::PeerId;
use dkn_p2p::DriaP2PProtocol;
use dkn_utils::get_current_time_nanos;
use eyre::{Context, Result};
use libsecp256k1::{recover, Message, RecoveryId, SecretKey, Signature};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::utils::crypto::sha256hash;
use crate::DRIA_COMPUTE_NODE_VERSION;

use super::crypto::public_key_to_peer_id;

/// A message within Dria Knowledge Network.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriaMessage {
    /// `base64` encoded data.
    pub payload: String,
    /// The topic of the message, derived from `TopicHash`
    pub topic: String,
    /// The version of the Dria Compute Node, e.g. `0.1.0`.
    pub version: String,
    /// Protocol name of the Dria Compute Node, e.g. `dria`.
    pub protocol: String,
    /// The timestamp of the message, in nanoseconds
    pub timestamp: u128,
    /// The 64 byte signature of the message, in `hex`.
    pub signature: String,
    /// The recovery id for the signature.
    pub recovery_id: u8,
}

impl DriaMessage {
    /// Creates a new Dria message.
    ///
    /// - `data` is converted to a bytes reference, and encoded into base64 to make up the `payload` within.
    /// - `topic` is the name of the [gossipsub topic](https://docs.libp2p.io/concepts/pubsub/overview/).
    /// - `protocol` is the protocol name, e.g. `dria`.
    /// - `signing_key` is the secret key to sign the message.
    pub(crate) fn new(
        data: impl AsRef<[u8]>,
        topic: impl ToString,
        protocol: &DriaP2PProtocol,
        signing_key: &SecretKey,
    ) -> Self {
        // base64 encode the data to obtain payload
        let payload = BASE64_STANDARD.encode(data);

        // sign the SHA256 hash of the payload
        let (signature, recovery_id) =
            libsecp256k1::sign(&Message::parse(&sha256hash(&payload)), signing_key);

        Self {
            payload,
            topic: topic.to_string(),
            protocol: protocol.name.to_string(),
            timestamp: get_current_time_nanos(),
            version: DRIA_COMPUTE_NODE_VERSION.to_string(),
            signature: hex::encode(signature.serialize()),
            recovery_id: recovery_id.serialize(),
        }
    }

    /// Decodes the base64 payload into bytes.
    #[inline(always)]
    pub(crate) fn decode_payload(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64_STANDARD.decode(&self.payload)
    }

    /// Decodes and parses the base64 payload into JSON for the provided type `T`.
    pub fn parse_payload<T: DeserializeOwned>(&self) -> Result<T> {
        let parsed = serde_json::from_slice::<T>(&self.decode_payload()?)?;
        Ok(parsed)
    }

    /// Checks if the payload is signed by the owner of one of the given peer ids.
    pub(crate) fn is_signed(&self, authorized_peerids: &HashSet<PeerId>) -> Result<bool> {
        let signature_bytes =
            hex::decode(&self.signature).wrap_err("could not decode signature hex")?;
        let signature = Signature::parse_standard_slice(&signature_bytes)
            .wrap_err("could not parse signature bytes")?;

        let recovery_id =
            RecoveryId::parse(self.recovery_id).wrap_err("could not decode recovery id")?;

        // verify signature w.r.t the body and the given public key
        let message = Message::parse(&sha256hash(&self.payload));

        let recovered_public_key = recover(&message, &signature, &recovery_id)?;
        let recovered_peer_id = public_key_to_peer_id(&recovered_public_key);

        Ok(authorized_peerids.contains(&recovered_peer_id))
    }

    /// Converts the message to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).wrap_err("could not serialize message")
    }
}

impl fmt::Display for DriaMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let payload_decoded = self
            .decode_payload()
            .unwrap_or(self.payload.as_bytes().to_vec());

        let payload_str = String::from_utf8(payload_decoded).unwrap_or(self.payload.clone());
        write!(
            f,
            "{} message for {} at {}\n{}",
            self.topic, self.protocol, self.timestamp, payload_str
        )
    }
}

impl TryFrom<&dkn_p2p::libp2p::gossipsub::Message> for DriaMessage {
    type Error = serde_json::Error;

    fn try_from(value: &dkn_p2p::libp2p::gossipsub::Message) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value.data)
    }
}

#[cfg(test)]
mod tests {
    use libsecp256k1::PublicKey;
    use rand::thread_rng;

    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestStruct {
        hello: String,
    }

    impl Default for TestStruct {
        fn default() -> Self {
            TestStruct {
                hello: "world".to_string(),
            }
        }
    }

    const TOPIC: &str = "test-topic";

    #[test]
    fn test_signed_message() {
        let mut rng = thread_rng();
        let sk = SecretKey::random(&mut rng);
        let pk = PublicKey::from_secret_key(&sk);
        let peer_id = public_key_to_peer_id(&pk);

        // create payload & message with signature & body
        let body = TestStruct::default();
        let body_str = serde_json::to_string(&body).unwrap();
        let message = DriaMessage::new(
            body_str,
            TOPIC,
            &DriaP2PProtocol::new_major_minor("test"),
            &sk,
        );

        // decode message
        let body = message
            .parse_payload::<TestStruct>()
            .expect("Should decode");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, TOPIC);
        assert_eq!(message.version, DRIA_COMPUTE_NODE_VERSION);
        assert!(message.timestamp > 0);

        let mut peer_ids = HashSet::new();
        peer_ids.insert(peer_id);
        assert!(
            message
                .is_signed(&peer_ids)
                .expect("Should verify signature"),
            "invalid signature"
        );

        let parsed_body = message.parse_payload().expect("Should decode");
        assert_eq!(body, parsed_body);
    }
}
