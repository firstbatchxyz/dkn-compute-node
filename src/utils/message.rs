use crate::utils::{
    crypto::{sha256hash, sign_bytes_recoverable},
    get_current_time_nanos,
    payload::TaskResponsePayload,
};

use base64::{prelude::BASE64_STANDARD, Engine};
use core::fmt;
use ecies::PublicKey;
use eyre::Result;
use libsecp256k1::SecretKey;
use serde::{Deserialize, Serialize};

/// A parsed message from gossipsub. When first received, the message data is simply a vector of bytes.
/// We treat that bytearray as a stringified JSON object, and parse it into this struct.
///
/// TODO: these are all available at protocol level as well
/// - payload is the data itself
/// - topic is available as TopicHash of Gossipsub
/// - version is given within the Identify protocol
/// - timestamp is available at protocol level via DataTransform
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct P2PMessage {
    pub(crate) payload: String,
    pub(crate) topic: String,
    pub(crate) version: String,
    pub(crate) timestamp: u128,
}

/// 65-byte signature as hex characters take up 130 characters.
/// The 65-byte signature is composed of 64-byte RSV signature and 1-byte recovery id.
///
/// When recovery is not required and only verification is being done, we omit the recovery id
/// and therefore use 128 characters: SIGNATURE_SIZE - 2.
const SIGNATURE_SIZE_HEX: usize = 130;

impl P2PMessage {
    /// Creates a new message with current timestamp and version equal to the crate version.
    ///
    /// - `payload` is gives as bytes. It is to be `base64` encoded internally.
    /// - `topic` is the name of the [gossipsub topic](https://docs.libp2p.io/concepts/pubsub/overview/).
    pub fn new(payload: impl AsRef<[u8]>, topic: &str) -> Self {
        Self {
            payload: BASE64_STANDARD.encode(payload),
            topic: topic.to_string(),
            version: crate::DRIA_COMPUTE_NODE_VERSION.to_string(),
            timestamp: get_current_time_nanos(),
        }
    }

    /// Creates a new Message by signing the SHA256 of the payload, and prepending the signature.
    pub fn new_signed(
        payload: impl AsRef<[u8]> + Clone,
        topic: &str,
        signing_key: &SecretKey,
    ) -> Self {
        let signature_bytes = sign_bytes_recoverable(&sha256hash(payload.clone()), signing_key);

        let mut signed_payload = Vec::new();
        signed_payload.extend_from_slice(signature_bytes.as_ref());
        signed_payload.extend_from_slice(payload.as_ref());
        Self::new(signed_payload, topic)
    }

    /// Creates the payload of a computation result, as per Dria Whitepaper section 5.1 algorithm 2:
    ///
    /// - Sign `task_id || payload` with node `self.secret_key`
    /// - Encrypt `result` with `task_public_key`
    pub fn new_signed_encrypted_payload(
        payload: impl AsRef<[u8]>,
        task_id: &str,
        encrypting_public_key: &[u8],
        signing_secret_key: &SecretKey,
    ) -> Result<TaskResponsePayload> {
        // sign payload
        let mut preimage = Vec::new();
        preimage.extend_from_slice(task_id.as_ref());
        preimage.extend_from_slice(payload.as_ref());
        let digest = libsecp256k1::Message::parse(&sha256hash(preimage));
        let (signature, recid) = libsecp256k1::sign(&digest, signing_secret_key);
        let signature: [u8; 64] = signature.serialize();
        let recid: [u8; 1] = [recid.serialize()];

        // encrypt payload
        let ciphertext = ecies::encrypt(encrypting_public_key, payload.as_ref())?;

        Ok(TaskResponsePayload {
            ciphertext: hex::encode(ciphertext),
            signature: format!("{}{}", hex::encode(signature), hex::encode(recid)),
            task_id: task_id.to_string(),
            timestamp: get_current_time_nanos(),
        })
    }

    /// Decodes the base64 payload into bytes.
    pub fn decode_payload(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64_STANDARD.decode(&self.payload)
    }

    /// Decodes and parses the payload into JSON.
    pub fn parse_payload<T: for<'a> Deserialize<'a>>(&self, signed: bool) -> Result<T> {
        let payload = self.decode_payload()?;

        let body = if signed {
            // skips the 65 byte hex signature
            &payload[SIGNATURE_SIZE_HEX..]
        } else {
            &payload[..]
        };

        let parsed: T = serde_json::from_slice(body)?;
        Ok(parsed)
    }

    /// Checks if the payload is signed by the given public key.
    pub fn is_signed(&self, public_key: &PublicKey) -> Result<bool> {
        // decode base64 payload
        let payload = self.decode_payload()?;

        // parse signature (64 bytes = 32 (x coord) + 32 (y coord))
        // skip the recovery id (1 byte)
        let (signature_hex, body) = (
            &payload[..SIGNATURE_SIZE_HEX - 2],
            &payload[SIGNATURE_SIZE_HEX..],
        );
        let signature_bytes = hex::decode(signature_hex).expect("could not decode");
        let signature = libsecp256k1::Signature::parse_standard_slice(&signature_bytes)
            .expect("could not parse");

        // verify signature
        let digest = libsecp256k1::Message::parse(&sha256hash(body));
        Ok(libsecp256k1::verify(&digest, &signature, public_key))
    }
}

impl fmt::Display for P2PMessage {
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

impl TryFrom<libp2p::gossipsub::Message> for P2PMessage {
    type Error = serde_json::Error;

    fn try_from(value: libp2p::gossipsub::Message) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{utils::crypto::sha256hash, DriaComputeNodeConfig};
    use ecies::decrypt;
    use libsecp256k1::SecretKey;
    use libsecp256k1::{verify, Message, PublicKey, RecoveryId, Signature};
    use rand::thread_rng;
    use serde_json::json;

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
    fn test_display_message() {
        let message = P2PMessage::new(b"hello world", "test-topic");
        println!("{}", message);
    }

    #[test]
    fn test_unsigned_message() {
        // create payload & message
        let body = TestStruct::default();
        let payload = serde_json::to_vec(&json!(body)).expect("Should serialize");
        let message = P2PMessage::new(payload, TOPIC);

        // decode message
        let message_body = message.decode_payload().expect("Should decode");
        let body = serde_json::from_slice::<TestStruct>(&message_body).expect("Should deserialize");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, "test-topic");
        assert_eq!(message.version, crate::DRIA_COMPUTE_NODE_VERSION);
        assert!(message.timestamp > 0);

        let parsed_body = message.parse_payload(false).expect("Should decode");
        assert_eq!(body, parsed_body);
    }

    #[test]
    fn test_signed_message() {
        let mut rng = thread_rng();
        let sk = SecretKey::random(&mut rng);
        let pk = PublicKey::from_secret_key(&sk);

        // create payload & message with signature & body
        let body = TestStruct::default();
        let body_str = serde_json::to_string(&body).unwrap();
        let message = P2PMessage::new_signed(body_str, TOPIC, &sk);

        // decode message
        let message_body = message.decode_payload().expect("Should decode");
        let body =
            serde_json::from_slice::<TestStruct>(&message_body[130..]).expect("Should parse");
        assert_eq!(
            serde_json::to_string(&body).expect("Should stringify"),
            "{\"hello\":\"world\"}"
        );
        assert_eq!(message.topic, "test-topic");
        assert_eq!(message.version, crate::DRIA_COMPUTE_NODE_VERSION);
        assert!(message.timestamp > 0);

        assert!(message.is_signed(&pk).expect("Should check signature"));

        let parsed_body = message.parse_payload(true).expect("Should decode");
        assert_eq!(body, parsed_body);
    }

    #[test]
    fn test_payload_generation_verification() {
        const TASK_SECRET_KEY_HEX: &[u8; 32] = b"aaaabbbbccccddddddddccccbbbbaaaa";
        const TASK_ID: &str = "12345678abcdef";
        const RESULT: &[u8; 28] = b"this is some result you know";

        let config = DriaComputeNodeConfig::default();
        let task_secret_key =
            SecretKey::parse(TASK_SECRET_KEY_HEX).expect("Should parse secret key");
        let task_public_key = PublicKey::from_secret_key(&task_secret_key);

        // create payload
        let payload = P2PMessage::new_signed_encrypted_payload(
            RESULT,
            TASK_ID,
            &task_public_key.serialize(),
            &config.secret_key,
        )
        .expect("Should create payload");

        // decrypt result
        let result = decrypt(
            &task_secret_key.serialize(),
            hex::decode(payload.ciphertext)
                .expect("Should decode")
                .as_slice(),
        )
        .expect("Could not decrypt");
        assert_eq!(result, RESULT, "Result mismatch");

        // verify signature
        let rsv = hex::decode(payload.signature).expect("Should decode");
        let mut signature_bytes = [0u8; 64];
        signature_bytes.copy_from_slice(&rsv[0..64]);
        let recid_bytes: [u8; 1] = [rsv[64]];
        let signature =
            Signature::parse_standard(&signature_bytes).expect("Should parse signature");
        let recid = RecoveryId::parse(recid_bytes[0]).expect("Should parse recovery id");

        let mut preimage = vec![];
        preimage.extend_from_slice(TASK_ID.as_bytes());
        preimage.extend_from_slice(&result);
        let message = Message::parse(&sha256hash(preimage));
        assert!(
            verify(&message, &signature, &config.public_key),
            "Could not verify"
        );

        // recover verifying key (public key) from signature
        let recovered_public_key =
            libsecp256k1::recover(&message, &signature, &recid).expect("Could not recover");
        assert_eq!(
            config.public_key, recovered_public_key,
            "Public key mismatch"
        );
    }
}
