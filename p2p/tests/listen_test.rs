use dkn_p2p::{DriaNodes, DriaP2PClient, DriaP2PProtocol};
use eyre::Result;
use libp2p_identity::Keypair;

#[tokio::test]
#[ignore = "run this manually"]
async fn test_listen_topic_once() -> Result<()> {
    const TOPIC: &str = "pong";

    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .filter_module("listen_test", log::LevelFilter::Debug)
        .filter_module("dkn_p2p", log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    let listen_addr = "/ip4/0.0.0.0/tcp/4001".parse()?;

    // prepare nodes
    let nodes = DriaNodes::new(dkn_p2p::DriaNetworkType::Community)
    .with_bootstrap_nodes(["/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4".parse()?])
    .with_relay_nodes(["/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa".parse()?]);

    // spawn P2P client in another task
    let (client, mut commander, mut msg_rx) = DriaP2PClient::new(
        Keypair::generate_secp256k1(),
        listen_addr,
        &nodes,
        DriaP2PProtocol::default(),
    )
    .expect("could not create p2p client");

    // spawn task
    let task_handle = tokio::spawn(async move { client.run().await });

    // subscribe to the given topic
    commander
        .subscribe(TOPIC)
        .await
        .expect("could not subscribe");

    // wait for a single gossipsub message on this topic
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

    // unsubscribe to the given topic
    commander
        .unsubscribe(TOPIC)
        .await
        .expect("could not unsubscribe");

    // close command channel
    commander.shutdown().await.expect("could not shutdown");

    // close message channel
    msg_rx.close();

    log::info!("Waiting for p2p task to finish...");
    task_handle.await?;

    log::info!("Done!");
    Ok(())
}
