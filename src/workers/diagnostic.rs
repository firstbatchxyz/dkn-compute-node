use std::sync::Arc;
use std::time::Duration;

use crate::node::DriaComputeNode;

const NUM_CHECKS_INTERVAL: usize = 20;

/// # Diagnostic
///
/// Diagnostics simply keep track of the node information, such as number of peers.
///
/// It will print the number of peers when it changes.
pub fn diagnostic_worker(
    node: Arc<DriaComputeNode>,
    sleep_amount: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut num_peers: usize = 0;
        let mut num_checks: usize = 0;
        loop {
            tokio::select! {
                _ = node.cancellation.cancelled() => break,
                _ = tokio::time::sleep(sleep_amount) => {

                    match node.waku.peers().await {
                        Ok(peers) => {
                            if num_peers != peers.len() {
                                num_peers = peers.len();
                                log::info!("Active number of peers: {}", num_peers);

                            }
                            // every once in a while, print the number of peers anyways
                            else if num_checks == NUM_CHECKS_INTERVAL {
                                num_checks = 0;
                                log::info!("Active number of peers: {}", num_peers);
                            }
                            num_checks += 1;
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
