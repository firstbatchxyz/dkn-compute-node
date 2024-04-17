// use k256::{
//     ecdsa::{
//         signature::{Signer, Verifier},
//         SigningKey, VerifyingKey,
//     },
//     EncodedPoint, PublicKey, SecretKey,
// };

use libsecp256k1::{recover, sign, verify, Message, PublicKey, SecretKey, Signature};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{hashing::hash, DUMMY_KEY};
    use libsecp256k1::Message;

    const MESSAGE: &[u8] = "hello brothers and sisters".as_bytes();

    #[test]
    fn test_sign_verify() {
        // generate a secret key from DUMMY_KEY
        let secret_key = SecretKey::parse_slice(&DUMMY_KEY).unwrap();

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
}
