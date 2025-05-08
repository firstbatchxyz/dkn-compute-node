use crate::crypto::sha256hash;

use super::SemanticVersion;
use base64::{prelude::BASE64_STANDARD, Engine};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

/// Message format for Dria network communication.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriaMessage {
    /// `base64` encoded message payload, can be decoded with [`Self::decode_payload`].
    ///
    /// This payload is signed by the sender, and the public key can be recovered from the signature
    /// using [`Self::recover_public_key`].
    pub payload: String,
    // Topic identifier derived from TopicHash
    pub topic: String,
    // Semantic version of Dria Compute Node, of the form `X.Y.Z`
    pub version: SemanticVersion,
    // Protocol identifier, e.g. "dria"
    pub protocol: String,
    // Message timestamp in nanoseconds
    pub timestamp: chrono::DateTime<chrono::Utc>,
    // 64-byte hex-encoded signature
    pub signature: String,
    // Signature recovery ID
    pub recovery_id: u8,
}

#[derive(Error, Debug)]
pub enum DriaMessageError {
    #[error("Could not decode payload: {0}")]
    DecodeError(base64::DecodeError),
    #[error("Could not parse message: {0}")]
    ParseError(serde_json::Error),
    #[error("Invalid header (expected {expected:?}, got {found:?})")]
    ProtocolMismatch { expected: String, found: String },
    #[error("Invalid header (expected {expected:?}, got {found:?})")]
    VersionMismatch {
        expected: SemanticVersion,
        found: SemanticVersion,
    },
    #[error("Invalid signature ({0})")]
    InvalidSignature(libsecp256k1::Error),
}

impl DriaMessage {
    /// Creates a new Dria message.
    ///
    /// - `data` is converted to a bytes reference, and encoded into base64 to make up the `payload` within.
    /// - `topic` is the name of the [gossipsub topic](https://docs.libp2p.io/concepts/pubsub/overview/).
    /// - `protocol` is the protocol name, e.g. `dria`.
    /// - `signing_key` is the secret key to sign the message.
    pub fn new_signed(
        data: impl AsRef<[u8]>,
        topic: impl ToString,
        protocol: String,
        signing_key: &libsecp256k1::SecretKey,
        version: SemanticVersion,
    ) -> Self {
        // base64 encode the data to obtain payload
        let payload = BASE64_STANDARD.encode(data);

        // sign the SHA256 hash of the payload
        let (signature, recovery_id) = libsecp256k1::sign(
            &libsecp256k1::Message::parse(&sha256hash(&payload)),
            signing_key,
        );

        Self {
            payload,
            topic: topic.to_string(),
            protocol,
            timestamp: chrono::Utc::now(),
            version,
            signature: hex::encode(signature.serialize()),
            recovery_id: recovery_id.serialize(),
        }
    }

    /// Parses a slice of bytes into a `DriaMessage`, and checks for protocol & network matches.
    pub fn from_slice_checked(
        data: &[u8],
        protocol: String,
        version: SemanticVersion,
    ) -> Result<DriaMessage, DriaMessageError> {
        let message: DriaMessage =
            serde_json::from_slice(data).map_err(DriaMessageError::ParseError)?;

        // ensure that protocol names match
        if protocol != message.protocol {
            Err(DriaMessageError::ProtocolMismatch {
                expected: protocol,
                found: message.protocol,
            })
        } else
        // ensure versions are compatible
        if !version.is_compatible(&message.version) {
            Err(DriaMessageError::VersionMismatch {
                expected: version,
                found: message.version,
            })
        } else {
            Ok(message)
        }
    }

    /// Decodes the base64 payload into bytes.
    #[inline(always)]
    pub fn decode_payload(&self) -> Result<Vec<u8>, DriaMessageError> {
        BASE64_STANDARD
            .decode(&self.payload)
            .map_err(DriaMessageError::DecodeError)
    }

    /// Decodes with [`Self::decode_payload`] and parses the decoded payload into JSON for the provided type `T`.
    #[inline(always)]
    pub fn parse_payload<T: DeserializeOwned>(&self) -> Result<T, DriaMessageError> {
        let decoded = self.decode_payload()?;
        serde_json::from_slice::<T>(&decoded).map_err(DriaMessageError::ParseError)
    }

    /// Recovers the signature from the message payload.
    ///
    /// This may be costly to do in a hot loop.
    #[inline(always)]
    pub fn recover_public_key(&self) -> Result<libsecp256k1::PublicKey, DriaMessageError> {
        let message = libsecp256k1::Message::parse(&sha256hash(&self.payload));

        // parse the signature and recovery ID
        let signature =
            libsecp256k1::Signature::parse_standard_slice(&hex::decode(&self.signature).unwrap())
                .map_err(DriaMessageError::InvalidSignature)?;
        let recovery_id = libsecp256k1::RecoveryId::parse(self.recovery_id)
            .map_err(DriaMessageError::InvalidSignature)?;

        // recover the public key from the signature
        libsecp256k1::recover(&message, &signature, &recovery_id)
            .map_err(DriaMessageError::InvalidSignature)
    }
}

impl From<&DriaMessage> for Vec<u8> {
    fn from(message: &DriaMessage) -> Self {
        serde_json::to_vec(message).expect("should not fail")
    }
}

impl From<DriaMessage> for Vec<u8> {
    fn from(message: DriaMessage) -> Self {
        (&message).into()
    }
}

impl std::fmt::Display for DriaMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let payload_decoded = self
            .decode_payload()
            .unwrap_or(self.payload.as_bytes().to_vec());

        let payload_str = String::from_utf8_lossy(&payload_decoded);
        write!(
            f,
            "{}/{} message at {}\n{}",
            self.protocol, self.topic, self.timestamp, payload_str
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecies::SecretKey;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestStruct {
        hello: String,
    }

    const TOPIC: &str = "test";

    #[test]
    fn test_signed_message() {
        const DUMMY_SECRET_KEY: &[u8; 32] = b"driadriadriadriadriadriadriadria";
        let sk = SecretKey::parse(DUMMY_SECRET_KEY).unwrap();

        // create payload & message with signature & body
        let body = TestStruct {
            hello: "hi there baby!".to_string(),
        };
        let body_str = serde_json::to_string(&body).unwrap();
        let message = DriaMessage::new_signed(
            body_str,
            TOPIC,
            "test".into(),
            &sk,
            SemanticVersion::default(),
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
        assert_eq!(message.version, SemanticVersion::default());
        assert!(message.timestamp != chrono::DateTime::<chrono::Utc>::default());

        let parsed_body = message.parse_payload().expect("Should decode");
        assert_eq!(body, parsed_body);
    }
}
