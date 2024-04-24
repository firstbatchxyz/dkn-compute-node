use dria_compute_node::{
    node::DriaComputeNode,
    utils::{
        crypto::{sha256hash, to_address},
        filter::FilterPayload,
    },
};
use ecies::decrypt;
use fastbloom_rs::{FilterBuilder, Membership};
use libsecp256k1::{recover, verify, Message, PublicKey, RecoveryId, SecretKey, Signature};

const ADMIN_PRIV_KEY: &[u8; 32] = b"aaaabbbbccccddddddddccccbbbbaaaa";

/// This test demonstrates the creation and parsing of a payload.
///
/// In DKN, the payload is created by Compute Node but parsed by the Admin Node.
/// At the end, there is also the verification step for the commitments.
#[test]
fn test_payload_generation_verification() {
    const RESULT: &[u8; 28] = b"this is some result you know";

    let node = DriaComputeNode::default();
    let secret_key = SecretKey::parse(ADMIN_PRIV_KEY).unwrap();
    let public_key = PublicKey::from_secret_key(&secret_key);

    // create payload
    let payload = node
        .create_payload(RESULT, &public_key.serialize())
        .unwrap();

    // (here we assume the payload is sent to Waku network, and picked up again)

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
        verify(&message, &signature, &node.config.DKN_WALLET_PUBLIC_KEY),
        "Could not verify."
    );

    // recover verifying key (public key) from signature
    let recovered_public_key =
        libsecp256k1::recover(&message, &signature, &recid).expect("Could not recover");
    assert_eq!(
        node.config.DKN_WALLET_PUBLIC_KEY, recovered_public_key,
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

/// This test demonstrates the process of heartbeat & task assignment.
///
/// A heart-beat message is sent over the network by Admin Node, and compute node responds with a signature.
#[test]
fn test_heartbeat_and_task_assignment() {
    let node = DriaComputeNode::default();

    // a heartbeat message is signed and sent to Admin Node (via Waku network)
    let heartbeat_message = Message::parse(&sha256hash(b"sign-me"));
    let (heartbeat_signature, heartbeat_recid) = node.sign(&heartbeat_message);

    // admin recovers the address from the signature
    let recovered_public_key = recover(&heartbeat_message, &heartbeat_signature, &heartbeat_recid)
        .expect("Could not recover");
    assert_eq!(
        node.config.DKN_WALLET_PUBLIC_KEY, recovered_public_key,
        "Public key mismatch."
    );
    let address = to_address(&recovered_public_key);
    assert_eq!(address, node.address(), "Address mismatch.");

    // admin node assigns the task to the compute node via Bloom Filter
    let mut bloom = FilterBuilder::new(100, 0.01).build_bloom_filter();
    bloom.add(&address);
    let filter_payload = FilterPayload::from(bloom);

    // compute node receives the filter and checks if it is tasked
    assert!(node.is_tasked(filter_payload), "Node should be tasked.");
}
