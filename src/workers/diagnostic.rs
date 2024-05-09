use std::time::Duration;
use std::sync::Arc;

use crate::node::DriaComputeNode;

/// # Diagnostic Worker
///
/// This is a simple worker that keeps track of the node information, and prints it at regular intervals.
/// In particular, it will print the number of peers.
pub fn diagnostic_worker(
    node: Arc<DriaComputeNode>,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = node.cancellation.cancelled() => break,
                _ = tokio::time::sleep(sleep_amount) => {

                    match node.waku.peers().await {
                        Ok(peers) => {
                            log::info!("Active number of peers: {}", peers.len());
                        },
                        Err(e) => {
                            log::error!("Error getting peers: {}", e);
                            continue;
                        }
                    };

                }
            }
        }
    })
}
