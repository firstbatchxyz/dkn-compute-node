use base64::{prelude::BASE64_STANDARD, Engine};
use core::fmt;
use dkn_p2p::DriaP2PProtocol;
use eyre::{Context, Result};
use libsecp256k1::{Message, SecretKey};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::utils::crypto::sha256hash;
use crate::DRIA_COMPUTE_NODE_VERSION;

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
    /// The timestamp of the message
    pub timestamp: chrono::DateTime<chrono::Utc>,
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
            timestamp: chrono::Utc::now(),
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

#[cfg(test)]
mod tests {
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
        assert!(message.timestamp != chrono::DateTime::<chrono::Utc>::default());

        let parsed_body = message.parse_payload().expect("Should decode");
        assert_eq!(body, parsed_body);
    }
}
