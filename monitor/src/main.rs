use dkn_compute::refresh_dria_nodes;
use dkn_p2p::{
    libp2p_identity::Keypair, DriaNetworkType, DriaNodes, DriaP2PClient, DriaP2PProtocol,
};
use tokio_util::sync::CancellationToken;

mod node;
use node::DriaMonitorNode;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().expect("could not load .env");

    env_logger::builder()
        .filter(None, log::LevelFilter::Off)
        .filter_module("dkn_p2p", log::LevelFilter::Warn)
        .filter_module("dkn_compute", log::LevelFilter::Info)
        .filter_module("dkn_monitor", log::LevelFilter::Info)
        .parse_default_env() // reads RUST_LOG variable
        .init();

    let network = std::env::var("DKN_NETWORK")
        .map(|s| DriaNetworkType::from(s.as_str()))
        .unwrap_or(DriaNetworkType::Pro);
    let mut nodes = DriaNodes::new(network);
    refresh_dria_nodes(&mut nodes).await?;

    // setup p2p client
    let listen_addr = "/ip4/0.0.0.0/tcp/4069".parse()?;
    log::info!("Listen Address: {}", listen_addr);
    let keypair = Keypair::generate_secp256k1();
    log::info!("PeerID: {}", keypair.public().to_peer_id());
    let (client, commander, msg_rx) = DriaP2PClient::new(
        keypair,
        listen_addr,
        nodes.bootstrap_nodes.into_iter(),
        nodes.relay_nodes.into_iter(),
        nodes.rpc_nodes.into_iter(),
        DriaP2PProtocol::new_major_minor(network.protocol_name()),
    )?;

    // spawn p2p task
    let token = CancellationToken::new();
    let p2p_handle = tokio::spawn(async move { client.run().await });

    // wait for SIGTERM & SIGINT signal in another thread
    let sig_token = token.clone();
    let sig_handle = tokio::spawn(async move {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm = signal(SignalKind::terminate()).unwrap(); // Docker sends SIGTERM
        let mut sigint = signal(SignalKind::interrupt()).unwrap(); // Ctrl+C sends SIGINT
        tokio::select! {
            _ = sigterm.recv() => log::warn!("Recieved SIGTERM"),
            _ = sigint.recv() => log::warn!("Recieved SIGINT"),
            _ = sig_token.cancelled() => return,
        };
        sig_token.cancel();
    });

    // create monitor node
    log::info!(
        "Monitoring {} network (protocol: {}).",
        network,
        network.protocol_name()
    );
    let mut monitor = DriaMonitorNode::new(commander, msg_rx);

    // setup monitor
    monitor.setup().await?;
    monitor.run(token).await;
    monitor.shutdown().await?;

    log::info!("Waiting for task handles...");
    p2p_handle.await?;
    sig_handle.await?;

    log::info!("Done!");
    Ok(())
}
