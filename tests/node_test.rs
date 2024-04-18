use dria_compute_node::{node::DriaComputeNode, utils::crypto::sha256hash};
use ecies::decrypt;
use libsecp256k1::{verify, Message, PublicKey, RecoveryId, SecretKey, Signature};

const RESULT: &[u8; 28] = b"this is some result you know";
const TASK_PRIV_KEY: &[u8; 32] = b"aaaabbbbccccddddddddccccbbbbaaaa";

#[test]
fn test_payload_generation_verification() {
    let node = DriaComputeNode::default();
    let secret_key = SecretKey::parse(TASK_PRIV_KEY).unwrap();
    let public_key = PublicKey::from_secret_key(&secret_key);

    let payload = node
        .create_payload(RESULT, &public_key.serialize())
        .unwrap();

    // decrypt result
    let result = decrypt(
        &secret_key.serialize(),
        hex::decode(payload.ciphertext).unwrap().as_slice(),
    )
    .expect("Could not decrypt.");
    assert_eq!(result, RESULT, "Result mismatch.");

    // verify signature
    let rsv = hex::decode(payload.signature).unwrap();
    let mut signature_bytes = [0u8; 64];
    signature_bytes.copy_from_slice(&rsv[0..64]);
    let recid_bytes: [u8; 1] = [rsv[64]];
    let signature = Signature::parse_standard(&signature_bytes).unwrap();
    let recid = RecoveryId::parse(recid_bytes[0]).unwrap();

    let result_digest = sha256hash(result);
    let message = Message::parse_slice(&result_digest).unwrap();
    assert!(
        verify(&message, &signature, &node.public_key),
        "Could not verify."
    );

    // recover verifying key (public key) from signature
    let recovered_public_key =
        libsecp256k1::recover(&message, &signature, &recid).expect("Could not recover");
    assert_eq!(
        node.public_key, recovered_public_key,
        "Public key mismatch."
    );

    // verify commitments (algorithm 4 in whitepaper)
    let mut preimage = Vec::new();
    preimage.extend_from_slice(&signature_bytes);
    preimage.extend_from_slice(&recid_bytes);
    preimage.extend_from_slice(&result_digest);
    assert_eq!(
        sha256hash(preimage),
        hex::decode(payload.commitment).unwrap().as_slice(),
        "Commitment mismatch."
    );
}
