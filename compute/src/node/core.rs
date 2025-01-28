use eyre::{eyre, Result};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::{node::PingpongHandler, utils::DriaMessage, DriaComputeNode};

impl DriaComputeNode {
    /// Runs the main loop of the compute node.
    /// This method is not expected to return until cancellation occurs for the given token.
    pub async fn run(&mut self, cancellation: CancellationToken) -> Result<()> {
        /// Number of seconds between refreshing for diagnostic prints.
        const DIAGNOSTIC_REFRESH_INTERVAL_SECS: u64 = 30;
        /// Number of seconds between refreshing the available nodes.
        const AVAILABLE_NODES_REFRESH_INTERVAL_SECS: u64 = 30 * 60; // 30 minutes

        // prepare durations for sleeps
        let mut diagnostic_refresh_interval =
            tokio::time::interval(Duration::from_secs(DIAGNOSTIC_REFRESH_INTERVAL_SECS));
        diagnostic_refresh_interval.tick().await; // move one tick
        let mut available_node_refresh_interval =
            tokio::time::interval(Duration::from_secs(AVAILABLE_NODES_REFRESH_INTERVAL_SECS));
        available_node_refresh_interval.tick().await; // move one tick

        // subscribe to topics
        self.subscribe(PingpongHandler::LISTEN_TOPIC).await?;
        self.subscribe(PingpongHandler::RESPONSE_TOPIC).await?;

        loop {
            tokio::select! {
                // a task is completed by the worker & should be responded to the requesting peer
                task_response_msg_opt = self.task_output_rx.recv() => {
                    let task_response_msg = task_response_msg_opt.ok_or(
                      eyre!("Publish channel closed unexpectedly, we still have {} batch and {} single tasks.", self.pending_tasks_batch.len(), self.pending_tasks_single.len())
                    )?; {
                        self.handle_task_response(task_response_msg).await?;
                    }
                },

                // a GossipSub message is received from the channel
                // this is expected to be sent by the p2p client
                gossipsub_msg_opt = self.gossip_message_rx.recv() => {
                    let (peer_id, message_id, message) = gossipsub_msg_opt.ok_or(eyre!("message_rx channel closed unexpectedly."))?;

                    // handle the message, returning a message acceptance for the received one
                    let acceptance = self.handle_message((peer_id, &message_id, message)).await;

                    // validate the message based on the acceptance
                    // cant do anything but log if this gives an error as well
                    if let Err(e) = self.p2p.validate_message(&message_id, &peer_id, acceptance).await {
                        log::error!("Error validating message {}: {:?}", message_id, e);
                    }

                },

                // a Request is received from the channel, sent by p2p client
                request_msg_opt = self.request_rx.recv() => {
                  let request = request_msg_opt.ok_or(eyre!("request_rx channel closed unexpectedly."))?;
                  if let Err(e) = self.handle_request(request).await {
                      log::error!("Error handling request: {:?}", e);
                  }
                },

                // check peer count every now and then
                _ = diagnostic_refresh_interval.tick() => self.handle_diagnostic_refresh().await,

                // available nodes are refreshed every now and then
                _ = available_node_refresh_interval.tick() => self.handle_available_nodes_refresh().await,

                // check if the cancellation token is cancelled
                // this is expected to be cancelled by the main thread with signal handling
                _ = cancellation.cancelled() => break,
            }
        }

        // unsubscribe from topics
        self.unsubscribe(PingpongHandler::LISTEN_TOPIC).await?;
        self.unsubscribe(PingpongHandler::RESPONSE_TOPIC).await?;

        // print one final diagnostic as a summary
        self.handle_diagnostic_refresh().await;

        // shutdown channels
        self.shutdown().await?;

        Ok(())
    }

    /// Shorthand method to create a signed message with the given data and topic.
    #[inline(always)]
    pub fn new_message(&self, data: impl AsRef<[u8]>, topic: impl ToString) -> DriaMessage {
        DriaMessage::new(data, topic, self.p2p.protocol(), &self.config.secret_key)
    }

    /// Shutdown channels between p2p, worker and yourself.
    ///
    /// Can be inlined as it is called only once from very few places.
    #[inline]
    pub async fn shutdown(&mut self) -> Result<()> {
        log::debug!("Sending shutdown command to p2p client.");
        self.p2p.shutdown().await?;

        log::debug!("Closing gossip message receipt channel.");
        self.gossip_message_rx.close();

        log::debug!("Closing task response channel.");
        self.task_output_rx.close();

        Ok(())
    }
}
