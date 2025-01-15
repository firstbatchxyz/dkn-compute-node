use dkn_p2p::{DriaNodes, DriaP2PClient, DriaP2PProtocol};
use eyre::Result;
use libp2p_identity::Keypair;

/// A gossipsub test that listens for a single message on a given topic.
/// Terminates when a message is received.
///
/// ## Run command
///
/// ```sh
/// cargo test --package dkn-p2p --test gossipsub_test --all-features -- test_gossipsub --exact --show-output --ignored
/// ```
#[tokio::test]
#[ignore = "run this manually"]
async fn test_gossipsub() -> Result<()> {
    const TOPIC: &str = "pong";

    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .filter_module("gossipsub_test", log::LevelFilter::Debug)
        .filter_module("dkn_p2p", log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    let listen_addr = "/ip4/0.0.0.0/tcp/4001".parse()?;

    // prepare nodes
    let nodes = DriaNodes::new(dkn_p2p::DriaNetworkType::Community)
    .with_bootstrap_nodes(["/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4".parse()?])
    .with_relay_nodes(["/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa".parse()?]);

    // spawn P2P client in another task
    let (client, mut commander, mut msg_rx, _) = DriaP2PClient::new(
        Keypair::generate_secp256k1(),
        listen_addr,
        &nodes,
        DriaP2PProtocol::default(),
    )?;
    let task_handle = tokio::spawn(async move { client.run().await });

    // wait for a single gossipsub message on this topic
    commander.subscribe(TOPIC).await?;
    log::info!("Waiting for messages...");
    let message = msg_rx.recv().await;
    match message {
        Some((peer, message_id, _)) => {
            log::info!("Received {} message {} from {}", TOPIC, message_id, peer);
        }
        None => {
            log::warn!("No message received for topic: {}", TOPIC);
        }
    }
    commander.unsubscribe(TOPIC).await?;

    // close everything
    commander.shutdown().await?;
    msg_rx.close();

    // wait for handle to return
    task_handle.await?;

    log::info!("Done!");
    Ok(())
}
