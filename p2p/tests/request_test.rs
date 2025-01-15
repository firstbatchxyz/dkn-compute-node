use std::str::FromStr;

use dkn_p2p::DriaNetworkType::Community;
use dkn_p2p::{DriaNodes, DriaP2PClient, DriaP2PProtocol};
use eyre::Result;
use libp2p::PeerId;
use libp2p_identity::Keypair;

/// Makes a dummy request to some peer hardcoded within the test.
///
/// ## Run command
///
/// ```sh
/// cargo test --package dkn-p2p --test request_test --all-features -- test_request_message --exact --show-output --ignored
/// ```
#[tokio::test]
#[ignore = "run this manually"]
async fn test_request_message() -> Result<()> {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .filter_module("request_test", log::LevelFilter::Debug)
        .filter_module("dkn_p2p", log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    let listen_addr = "/ip4/0.0.0.0/tcp/4001".parse()?;

    // prepare nodes
    let nodes = DriaNodes::new(Community)
        .with_bootstrap_nodes(Community.get_static_bootstrap_nodes())
        .with_relay_nodes(Community.get_static_relay_nodes());

    // spawn P2P client in another task
    let (client, mut commander, mut msg_rx, mut req_rx) = DriaP2PClient::new(
        Keypair::generate_secp256k1(),
        listen_addr,
        &nodes,
        DriaP2PProtocol::default(),
    )
    .expect("could not create p2p client");

    // spawn task
    let task_handle = tokio::spawn(async move { client.run().await });

    log::info!("Waiting a bit until we have enough peers");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    let peer_id =
        PeerId::from_str("16Uiu2HAmB5HGdwLNHX81u7ey1fvDx5Mr4ofa2PdSSVxFKrrcErAN").unwrap();
    log::info!("Making a request to peer: {}", peer_id);
    commander
        .request(peer_id, b"here is some data".into())
        .await?;

    log::info!("Waiting for response logs for a few moments...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // close command channel
    commander.shutdown().await.expect("could not shutdown");

    // close other channels
    msg_rx.close();
    req_rx.close();

    log::info!("Waiting for p2p task to finish...");
    task_handle.await?;

    log::info!("Done!");
    Ok(())
}
