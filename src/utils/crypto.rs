use bloomfilter::Bloom;
use ecies::{decrypt, encrypt};
use libsecp256k1::{recover, sign, verify, Message, PublicKey, RecoveryId, SecretKey, Signature};
use sha2::{Digest, Sha256};

/// Generic SHA256 hash function.
#[inline]
pub fn hash(data: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Explicit form of a secp256k1 signature.
///
/// A `From` trait is implemented for a (Signature, RecoveryId) pair.
pub struct SignatureVRS {
    v: u8,
    r: [u8; 32],
    s: [u8; 32],
}

impl From<(Signature, RecoveryId)> for SignatureVRS {
    #[inline]
    fn from(value: (Signature, RecoveryId)) -> Self {
        let (v, r, s) = (value.1.serialize(), value.0.r.b32(), value.0.s.b32());
        Self { v, r, s }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    const DUMMY_KEY: &[u8; 32] = b"driadriadriadriadriadriadriadria";
    const MESSAGE: &[u8] = "hello world".as_bytes();

    #[test]
    fn test_hash() {
        // sha256 of "hello world"
        let expected =
            hex!("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9").as_slice();
        assert_eq!(hash(MESSAGE), expected);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let sk = SecretKey::parse_slice(DUMMY_KEY).expect("Could not parse private key slice.");
        let pk = PublicKey::from_secret_key(&sk);
        let (sk, pk) = (&sk.serialize(), &pk.serialize());

        let ciphertext = encrypt(pk, MESSAGE).expect("Could not encrypt.");
        let plaintext = decrypt(sk, &ciphertext).expect("Could not decyrpt.");
        assert_eq!(MESSAGE, plaintext.as_slice());
    }

    #[test]
    fn test_sign_verify() {
        let secret_key =
            SecretKey::parse_slice(DUMMY_KEY).expect("Could not parse private key slice.");

        // sign the message using the secret key
        let digest = hash(MESSAGE);
        let message = Message::parse_slice(&digest).expect("Could not parse message.");
        let (signature, recid) = sign(&message, &secret_key);

        // recover verifying key (public key) from signature
        let expected_public_key = PublicKey::from_secret_key(&secret_key);
        let recovered_public_key =
            recover(&message, &signature, &recid).expect("Could not recover");
        assert_eq!(expected_public_key, recovered_public_key);

        // verify the signature
        let public_key = recovered_public_key;
        let ok = verify(&message, &signature, &public_key);
        assert!(ok, "could not verify");
    }

    #[test]
    fn test_bloom_filter() {
        let num_items = 128;
        let fp_rate = 0.001;

        let mut bloom = Bloom::new_for_fp_rate(num_items, fp_rate);
        bloom.set(&10);

        assert_eq!(bloom.check(&10), true);
        assert_eq!(bloom.check(&20), false);

        // println!("{:?}", bloom.bitmap().len());
    }
}
