use ecies::{decrypt, encrypt, utils::generate_keypair};
use ecies::{PublicKey, SecretKey};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::DUMMY_KEY;
    const MESSAGE: &[u8] = "hello brothers and sisters".as_bytes();

    #[test]
    fn test_encrypt_decrypt_ecies() {
        let sk = SecretKey::parse_slice(&DUMMY_KEY).expect("Could not parse private key slice.");
        let pk = PublicKey::from_secret_key(&sk);
        let (sk, pk) = (&sk.serialize(), &pk.serialize());

        let ciphertext = encrypt(pk, MESSAGE).expect("Could not encrypt.");
        let plaintext = decrypt(sk, &ciphertext).expect("Could not decyrpt.");
        assert_eq!(MESSAGE, plaintext.as_slice());
    }
}
