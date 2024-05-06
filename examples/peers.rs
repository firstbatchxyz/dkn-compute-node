use dkn_compute::{config::DriaComputeNodeConfig, node::DriaComputeNode};

#[tokio::main]
async fn main() {
    let node = DriaComputeNode::new(DriaComputeNodeConfig::new());
    let waku = node.waku;

    let peers = waku.peers().await.unwrap();

    println!("Connected to {} peers:", peers.len());
    for peer in peers {
        println!("  {}", peer.multiaddr);
    }
}
