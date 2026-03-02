use libsecp256k1::{PublicKey, SecretKey};
use sha2::{Digest as _, Sha256};
use sha3::Keccak256;

use crate::error::NodeError;

/// Node identity derived from a secp256k1 secret key.
/// The address is an Ethereum-style address (last 20 bytes of keccak256 of uncompressed pubkey).
pub struct Identity {
    pub secret_key: SecretKey,
    #[allow(dead_code)]
    pub public_key: PublicKey,
    #[allow(dead_code)]
    pub address: [u8; 20],
    pub address_hex: String,
}

impl Identity {
    /// Create an identity from a hex-encoded secret key (without 0x prefix).
    pub fn from_secret_hex(hex_str: &str) -> Result<Self, NodeError> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| NodeError::Identity(format!("invalid hex: {e}")))?;
        let secret_key = SecretKey::parse_slice(&bytes)
            .map_err(|e| NodeError::Identity(format!("invalid secret key: {e}")))?;
        let public_key = PublicKey::from_secret_key(&secret_key);
        let address = public_key_to_address(&public_key);
        let address_hex = hex::encode(address);

        Ok(Identity {
            secret_key,
            public_key,
            address,
            address_hex,
        })
    }

    /// Sign a SHA-256 digest of the given message.
    /// Returns (signature, recovery_id).
    pub fn sign(&self, message: &[u8]) -> (libsecp256k1::Signature, libsecp256k1::RecoveryId) {
        let digest = sha256hash(message);
        let msg = libsecp256k1::Message::parse_slice(&digest)
            .expect("SHA-256 output is always 32 bytes");
        libsecp256k1::sign(&msg, &self.secret_key)
    }
}

/// SHA-256 hash.
#[inline(always)]
pub fn sha256hash(data: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Keccak-256 hash.
#[inline(always)]
pub fn keccak256hash(data: impl AsRef<[u8]>) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// Derive an Ethereum address from a secp256k1 public key.
/// Serializes uncompressed (65 bytes: 0x04 || x || y), hashes (x || y) with keccak256,
/// and takes the last 20 bytes.
#[inline]
fn public_key_to_address(public_key: &PublicKey) -> [u8; 20] {
    let public_key_xy = &public_key.serialize()[1..];
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&keccak256hash(public_key_xy)[12..32]);
    addr
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMMY_SECRET_KEY: &[u8; 32] = b"driadriadriadriadriadriadriadria";

    #[test]
    fn test_sha256() {
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert_eq!(hex::encode(sha256hash(b"hello world")), expected);
    }

    #[test]
    fn test_address_from_secret() {
        let hex_key = hex::encode(DUMMY_SECRET_KEY);
        let identity = Identity::from_secret_hex(&hex_key).unwrap();
        assert_eq!(
            identity.address_hex,
            "d79fdf178547614cfdd0df6397c53569716bd596"
        );
    }

    #[test]
    fn test_sign_and_recover() {
        let hex_key = hex::encode(DUMMY_SECRET_KEY);
        let identity = Identity::from_secret_hex(&hex_key).unwrap();

        let message = b"hello world";
        let (signature, recid) = identity.sign(message);

        // Recover public key from signature
        let digest = sha256hash(message);
        let msg = libsecp256k1::Message::parse_slice(&digest).unwrap();
        let recovered = libsecp256k1::recover(&msg, &signature, &recid).unwrap();
        assert_eq!(recovered, identity.public_key);
    }

    #[test]
    fn test_invalid_hex() {
        assert!(Identity::from_secret_hex("not-hex").is_err());
    }

    #[test]
    fn test_invalid_key_length() {
        assert!(Identity::from_secret_hex("abcd").is_err());
    }
}
