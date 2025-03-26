use dkn_p2p::libp2p::{Multiaddr, PeerId};
use eyre::{eyre, Result};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::{utils::DriaMessage, DriaComputeNode};

impl DriaComputeNode {
    /// Runs the main loop of the compute node.
    /// This method is not expected to return until cancellation occurs for the given token.
    pub async fn run(&mut self, cancellation: CancellationToken) {
        /// Number of seconds between refreshing for diagnostic prints.
        const DIAGNOSTIC_REFRESH_INTERVAL_SECS: u64 = 30;
        /// Number of seconds between refreshing the available nodes.
        const AVAILABLE_NODES_REFRESH_INTERVAL_SECS: u64 = 10 * 60;
        /// Number of seconds between each heartbeat sent to the RPC.
        const HEARTBEAT_INTERVAL_SECS: u64 = 60;

        // prepare durations for sleeps
        let mut diagnostic_refresh_interval =
            tokio::time::interval(Duration::from_secs(DIAGNOSTIC_REFRESH_INTERVAL_SECS));
        let mut available_node_refresh_interval =
            tokio::time::interval(Duration::from_secs(AVAILABLE_NODES_REFRESH_INTERVAL_SECS));
        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

        // move each one tick
        available_node_refresh_interval.tick().await;
        diagnostic_refresh_interval.tick().await;
        heartbeat_interval.tick().await;

        loop {
            tokio::select! {
                // a task is completed by the worker & should be responded to the requesting peer
                task_response_msg_opt = self.task_output_rx.recv() => {
                    if let Some(task_response_msg) = task_response_msg_opt {
                        if let Err(e) = self.send_task_output(task_response_msg).await {
                            log::error!("Error responding to task: {:?}", e);
                        }
                    } else {
                        log::error!("task_output_rx channel closed unexpectedly, we still have {} batch and {} single tasks.", self.pending_tasks_batch.len(), self.pending_tasks_single.len());
                        break;
                    }
                },

                // a Request or Response is received by the p2p client
                reqres_msg_opt = self.reqres_rx.recv() => {
                  if let Some((peer_id, message)) = reqres_msg_opt {
                    self.handle_reqres(peer_id, message).await;
                  } else {
                    log::error!("reqres_rx channel closed unexpectedly.");
                    break;
                  }
                },

                // check peer count every now and then
                _ = diagnostic_refresh_interval.tick() => self.handle_diagnostic_refresh().await,

                // available nodes are refreshed every now and then
                _ = available_node_refresh_interval.tick() => self.handle_available_nodes_refresh().await,

                _ = heartbeat_interval.tick() => {
                  if let Err(e) = self.send_heartbeat().await {
                    log::error!("Error making heartbeat: {:?}", e);
                }
              },

                // check if the cancellation token is cancelled
                // this is expected to be cancelled by the main thread with signal handling
                _ = cancellation.cancelled() => {
                    log::info!("Cancellation received, shutting down the node.");
                    break;
                },
            }
        }

        // print one final diagnostic as a summary
        self.handle_diagnostic_refresh().await;

        // shutdown channels
        if let Err(e) = self.shutdown().await {
            log::error!("Could not shutdown the node gracefully: {:?}", e);
        }
    }

    /// Shorthand method to create a signed message with the given data and topic.
    ///
    /// Topic was previously used for GossipSub, but kept for verbosity.
    #[inline(always)]
    pub fn new_message(&self, data: impl AsRef<[u8]>, topic: impl ToString) -> DriaMessage {
        DriaMessage::new(data, topic, self.p2p.protocol(), &self.config.secret_key)
    }

    /// Dial the given peer at the given address.
    pub async fn dial_with_timeout(&mut self, peer_id: PeerId, addr: Multiaddr) -> Result<()> {
        // while not yet known, some people get stuck during the dialling step,
        // this timeout prevents that.
        const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

        match tokio::time::timeout(DIAL_TIMEOUT, self.p2p.dial(peer_id, addr)).await {
            Err(timeout) => Err(eyre!("Timeout dialling RPC node: {}", timeout)),
            Ok(result) => result, // this is also a `Result` enum
        }
    }

    /// Shutdown channels between p2p, worker and yourself.
    ///
    /// Can be inlined as it is called only once from very few places.
    #[inline]
    pub async fn shutdown(&mut self) -> Result<()> {
        log::debug!("Sending shutdown command to p2p client.");
        self.p2p.shutdown().await?;

        log::debug!("Closing task output channel.");
        self.task_output_rx.close();

        log::debug!("Closing reqres channel.");
        self.reqres_rx.close();

        Ok(())
    }
}
