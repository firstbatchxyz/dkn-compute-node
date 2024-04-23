use ecies::PublicKey;
use sha2::{Digest, Sha256};
use sha3::Keccak256;

/// Generic SHA256 function.
#[inline]
pub fn sha256hash(data: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// Generic KECCAK256 function.
#[inline]
pub fn keccak256hash(data: impl AsRef<[u8]>) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// Given a secp256k1 public key, finds the corresponding Ethereum address.
pub fn to_address(public_key: &PublicKey) -> [u8; 20] {
    // hash the public key (x,y)
    let public_key = public_key.serialize();
    let hash = keccak256hash(public_key.split_at(1).1);

    // get last 20 bytes
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecies::{decrypt, encrypt};
    use hex::decode;
    use libsecp256k1::{recover, sign, verify, Message, PublicKey, SecretKey};

    const DUMMY_KEY: &[u8; 32] = b"driadriadriadriadriadriadriadria";
    const MESSAGE: &[u8] = "hello world".as_bytes();

    #[test]
    fn test_hash() {
        // sha256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let expected = decode(expected).unwrap();
        assert_eq!(sha256hash(MESSAGE), expected.as_slice());
    }

    #[test]
    fn test_address() {
        let sk = SecretKey::parse_slice(DUMMY_KEY).expect("Could not parse private key slice.");
        let pk = PublicKey::from_secret_key(&sk);
        let addr = to_address(&pk);
        assert_eq!(
            "D79Fdf178547614CFdd0dF6397c53569716Bd596".to_lowercase(),
            hex::encode(addr)
        );
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
        let digest = sha256hash(MESSAGE);
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
}
