use dkn_p2p::DriaP2PClient;
use eyre::Result;
use libp2p::Multiaddr;
use libp2p_identity::Keypair;
use std::{env, str::FromStr};

const LOG_LEVEL: &str = "none,dkn_p2p=debug";

#[tokio::test]
#[ignore = "run manually with logs"]
async fn test_listen_topic_once() -> Result<()> {
    // topic to be listened to
    const TOPIC: &str = "pong";

    env::set_var("RUST_LOG", LOG_LEVEL);
    let _ = env_logger::try_init();

    // setup client
    let keypair = Keypair::generate_secp256k1();
    let addr = Multiaddr::from_str("/ip4/0.0.0.0/tcp/4001")?;
    let bootstraps = vec![Multiaddr::from_str(
        "/ip4/44.206.245.139/tcp/4001/p2p/16Uiu2HAm4q3LZU2T9kgjKK4ysy6KZYKLq8KiXQyae4RHdF7uqSt4",
    )?];
    let relays = vec![Multiaddr::from_str(
        "/ip4/34.201.33.141/tcp/4001/p2p/16Uiu2HAkuXiV2CQkC9eJgU6cMnJ9SMARa85FZ6miTkvn5fuHNufa",
    )?];
    let mut client = DriaP2PClient::new(keypair, addr, &bootstraps, &relays, "0.2")?;

    // subscribe to the given topic
    client.subscribe(TOPIC)?;

    // wait for a single gossipsub message on this topic
    let message = client.process_events().await;
    log::info!("Received {} message: {:?}", TOPIC, message);

    // unsubscribe gracefully
    client.unsubscribe(TOPIC)?;

    Ok(())
}
