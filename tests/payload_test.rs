use dkn_compute::{node::DriaComputeNode, utils::crypto::sha256hash};
use ecies::decrypt;
use libsecp256k1::{verify, Message, PublicKey, RecoveryId, SecretKey, Signature};

/// This test demonstrates the creation and parsing of a payload.
///
/// In DKN, the payload is created by Compute Node but parsed by the Admin Node.
/// At the end, there is also the verification step for the commitments.
#[test]
fn test_payload_generation_verification() {
    const TASK_SECRET_KEY_HEX: &[u8; 32] = b"aaaabbbbccccddddddddccccbbbbaaaa";
    const TASK_ID: &str = "12345678abcdef";
    const RESULT: &[u8; 28] = b"this is some result you know";

    let node = DriaComputeNode::default();
    let task_secret_key = SecretKey::parse(TASK_SECRET_KEY_HEX).expect("Should parse secret key");
    let task_public_key = PublicKey::from_secret_key(&task_secret_key);

    // create payload
    let payload = node
        .create_payload(RESULT, TASK_ID, &task_public_key.serialize())
        .expect("Should create payload");

    // (here we assume the payload is sent to Waku network, and picked up again)

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
    let signature = Signature::parse_standard(&signature_bytes).expect("Should parse signature");
    let recid = RecoveryId::parse(recid_bytes[0]).expect("Should parse recovery id");

    let mut preimage = vec![];
    preimage.extend_from_slice(TASK_ID.as_bytes());
    preimage.extend_from_slice(&result);
    let message = Message::parse(&sha256hash(preimage));
    assert!(
        verify(&message, &signature, &node.config.public_key),
        "Could not verify"
    );

    // recover verifying key (public key) from signature
    let recovered_public_key =
        libsecp256k1::recover(&message, &signature, &recid).expect("Could not recover");
    assert_eq!(
        node.config.public_key, recovered_public_key,
        "Public key mismatch"
    );
}
