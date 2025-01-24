use base64::{prelude::BASE64_STANDARD, Engine};
use core::fmt;
use dkn_p2p::libp2p::PeerId;
use dkn_utils::get_current_time_nanos;
use eyre::{Context, Result};
use libsecp256k1::{recover, Message, RecoveryId, SecretKey, Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::utils::crypto::{sha256hash, sign_bytes_recoverable};
use crate::DRIA_COMPUTE_NODE_VERSION;

use super::crypto::public_key_to_peer_id;

/// A message within Dria Knowledge Network.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriaMessage {
    /// Base64 encoded payload, stores the main result.
    pub payload: String,
    /// The topic of the message, derived from `TopicHash`
    ///
    /// NOTE: This can be obtained via `TopicHash` in GossipSub
    pub topic: String,
    /// The version of the Dria Compute Node
    ///
    /// NOTE: This can be obtained via Identify protocol version
    pub version: String,
    /// Identity protocol string of the Dria Compute Node
    #[serde(default)]
    pub identity: String,
    /// The timestamp of the message, in nanoseconds
    ///
    /// NOTE: This can be obtained via `DataTransform` in GossipSub
    pub timestamp: u128,
}

/// 65-byte signature as hex characters take up 130 characters.
/// The 65-byte signature is composed of 64-byte RSV signature and 1-byte recovery id.
///
/// When recovery is not required and only verification is being done, we omit the recovery id
/// and therefore use 128 characters: SIGNATURE_SIZE - 2.
const SIGNATURE_SIZE_HEX: usize = 130;

impl DriaMessage {
    /// Creates a new message with current timestamp and version equal to the crate version.
    ///
    /// - `data` is given as bytes, it is encoded into base64 to make up the `payload` within.
    /// - `topic` is the name of the [gossipsub topic](https://docs.libp2p.io/concepts/pubsub/overview/).
    pub(crate) fn new(data: impl AsRef<[u8]>, topic: &str) -> Self {
        Self {
            payload: BASE64_STANDARD.encode(data),
            topic: topic.to_string(),
            version: DRIA_COMPUTE_NODE_VERSION.to_string(),
            identity: String::default(),
            timestamp: get_current_time_nanos(),
        }
    }

    /// Creates a new Message by signing the SHA256 of the payload, and prepending the signature.
    pub(crate) fn new_signed(data: impl AsRef<[u8]>, topic: &str, signing_key: &SecretKey) -> Self {
        // sign the SHA256 hash of the data
        let signature_bytes = sign_bytes_recoverable(&sha256hash(data.as_ref()), signing_key);

        // prepend the signature to the data, to obtain `signature || data` bytes
        let mut signed_data = Vec::new();
        signed_data.extend_from_slice(signature_bytes.as_ref());
        signed_data.extend_from_slice(data.as_ref());

        // create the actual message with this signed data
        Self::new(signed_data, topic)
    }

    /// Sets the identity of the message.
    pub(crate) fn with_identity(mut self, identity: String) -> Self {
        self.identity = identity;
        self
    }

    /// Decodes the base64 payload into bytes.
    #[inline(always)]
    pub(crate) fn decode_payload(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64_STANDARD.decode(&self.payload)
    }

    /// Decodes and parses the base64 payload into JSON for the provided type `T`.
    pub fn parse_payload<T: for<'a> Deserialize<'a>>(&self, signed: bool) -> Result<T> {
        let payload = self.decode_payload()?;

        let body = if signed {
            // skips the 65 byte hex signature
            &payload[SIGNATURE_SIZE_HEX..]
        } else {
            &payload[..]
        };

        let parsed = serde_json::from_slice::<T>(body)?;
        Ok(parsed)
    }

    /// Checks if the payload is signed by the owner of one of the given peer ids.
    pub(crate) fn is_signed(&self, authorized_peerids: &HashSet<PeerId>) -> Result<bool> {
        // decode base64 payload
        let data = self.decode_payload()?;

        // parse signature from the following bytes:
        //    32   +   32  +     1      +  ...
        // (  x   ||   y   ||  rec_id  || data
        let (signature_hex, rec_id_hex, body) = (
            &data[..SIGNATURE_SIZE_HEX - 2],
            &data[SIGNATURE_SIZE_HEX - 2..SIGNATURE_SIZE_HEX],
            &data[SIGNATURE_SIZE_HEX..],
        );
        let signature_bytes =
            hex::decode(signature_hex).wrap_err("could not decode signature hex")?;
        let recovery_id_bytes = hex::decode(rec_id_hex).wrap_err("could not decode rec id hex")?;

        // now obtain the signature itself
        let signature = Signature::parse_standard_slice(&signature_bytes)
            .wrap_err("could not parse signature bytes")?;
        let recovery_id =
            RecoveryId::parse(recovery_id_bytes[0]).wrap_err("could not decode recovery id")?;

        // verify signature w.r.t the body and the given public key
        let message = Message::parse(&sha256hash(body));

        let recovered_public_key = recover(&message, &signature, &recovery_id)?;
        let recovered_peer_id = public_key_to_peer_id(&recovered_public_key);

        Ok(authorized_peerids.contains(&recovered_peer_id))
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
            "{} message at {}\n{}",
            self.topic, self.timestamp, payload_str
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
    use ecies::PublicKey;
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
    #[ignore = "run manually"]
    fn test_display_message() {
        let message = DriaMessage::new(b"hello world", TOPIC);
        println!("{}", message);
    }

    #[test]
    fn test_unsigned_message() {
        // create payload & message
        let body = TestStruct::default();
        let data = serde_json::to_vec(&body).expect("Should serialize");
        let message = DriaMessage::new(data, TOPIC);

        // decode message
        let message_body = message.decode_payload().expect("Should decode");
        let body = serde_json::from_slice::<TestStruct>(&message_body).expect("Should deserialize");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, TOPIC);
        assert_eq!(message.version, DRIA_COMPUTE_NODE_VERSION);
        assert!(message.timestamp > 0);

        // decode payload without signature
        let parsed_body = message.parse_payload(false).expect("Should decode");
        assert_eq!(body, parsed_body);
    }

    #[test]
    fn test_signed_message() {
        let mut rng = thread_rng();
        let sk = SecretKey::random(&mut rng);
        let pk = PublicKey::from_secret_key(&sk);
        let peer_id = public_key_to_peer_id(&pk);

        // create payload & message with signature & body
        let body = TestStruct::default();
        let body_str = serde_json::to_string(&body).unwrap();
        let message = DriaMessage::new_signed(body_str, TOPIC, &sk);

        // decode message
        let message_body = message.decode_payload().expect("Should decode");
        let body =
            serde_json::from_slice::<TestStruct>(&message_body[130..]).expect("Should parse");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, TOPIC);
        assert_eq!(message.version, DRIA_COMPUTE_NODE_VERSION);
        assert!(message.timestamp > 0);

        let mut peer_ids = HashSet::new();
        peer_ids.insert(peer_id);
        assert!(message
            .is_signed(&peer_ids)
            .expect("Should verify signature"));

        let parsed_body = message.parse_payload(true).expect("Should decode");
        assert_eq!(body, parsed_body);
    }
}
