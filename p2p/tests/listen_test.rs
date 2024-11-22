use dkn_p2p::{DriaP2PClient, DriaP2PProtocol};
use eyre::Result;
use libp2p::Multiaddr;
use libp2p_identity::Keypair;
use std::{env, str::FromStr};
use tokio_util::sync::CancellationToken;

const LOG_LEVEL: &str = "none,dkn_p2p=debug";

#[tokio::test]
#[ignore = "run manually with logs"]
async fn test_listen_topic_once() -> Result<()> {
    // topic to be listened to
    const TOPIC: &str = "pong";

    env::set_var("RUST_LOG", LOG_LEVEL);
    let _ = env_logger::builder().is_test(true).try_init();

    // setup client
    let keypair = Keypair::generate_secp256k1();
    let addr = Multiaddr::from_str("/ip4/0.0.0.0/tcp/4001")?;
    let bootstraps = vec![Multiaddr::from_str(
        "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4",
    )?];
    let relays = vec![Multiaddr::from_str(
        "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa",
    )?];
    let protocol = DriaP2PProtocol::new_major_minor("dria");

    // Create channel for P2P events
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    // Spawn P2P event processing task
    let cancellation = CancellationToken::new();
    let p2p_cancellation = cancellation.clone();
    let p2p_task = tokio::spawn(async move {
        let mut client: DriaP2PClient = DriaP2PClient::new(
            keypair,
            addr,
            bootstraps.into_iter(),
            relays.into_iter(),
            protocol,
            tx,
        )
        .expect("could not create p2p client");

        // subscribe to the given topic
        client.subscribe(TOPIC).expect("could not subscribe");

        tokio::select! {
            _ = client.process_events() => {
            },
            _ = p2p_cancellation.cancelled() => {
                client.unsubscribe(TOPIC).expect("could not unsubscribe");
            },
        };
    });

    // wait for a single gossipsub message on this topic
    log::info!("Waiting for messages...");
    let message = rx.recv().await;
    match message {
        Some((peer, message_id, _)) => {
            log::info!("Received {} message {} from {}", TOPIC, message_id, peer);
        }
        None => {
            log::warn!("No message received for topic: {}", TOPIC);
        }
    }

    cancellation.cancel();
    rx.close();

    log::info!("Waiting for p2p task to finish...");
    p2p_task.await?;

    Ok(())
}
