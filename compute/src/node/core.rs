use eyre::Result;
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
                // a Workflow message to be published is received from the channel
                // this is expected to be sent by the workflow worker
                publish_msg_opt = self.publish_rx.recv() => {
                    if let Some(publish_msg) = publish_msg_opt {
                        // remove the task from pending tasks based on its batchability
                        match publish_msg.batchable {
                            true => {
                                self.completed_tasks_batch += 1;
                                self.pending_tasks_batch.remove(&publish_msg.task_id);
                            },
                            false => {
                                self.completed_tasks_single += 1;
                                self.pending_tasks_single.remove(&publish_msg.task_id);
                            }
                        };

                        // publish the message
                        todo!("will respond via req-res");
                        // WorkflowHandler::handle_publish(self, publish_msg).await?;
                    } else {
                        log::error!("Publish channel closed unexpectedly.");
                        break;
                    };
                },

                // check peer count every now and then
                _ = diagnostic_refresh_interval.tick() => self.handle_diagnostic_refresh().await,

                // available nodes are refreshed every now and then
                _ = available_node_refresh_interval.tick() => self.handle_available_nodes_refresh().await,

                // a GossipSub message is received from the channel
                // this is expected to be sent by the p2p client
                gossipsub_msg_opt = self.message_rx.recv() => {
                    if let Some((peer_id, message_id, message)) = gossipsub_msg_opt {
                        // handle the message, returning a message acceptance for the received one
                        let acceptance = self.handle_message((peer_id, &message_id, message)).await;

                        // validate the message based on the acceptance
                        // cant do anything but log if this gives an error as well
                        if let Err(e) = self.p2p.validate_message(&message_id, &peer_id, acceptance).await {
                            log::error!("Error validating message {}: {:?}", message_id, e);
                        }
                    } else {
                        log::error!("message_rx channel closed unexpectedly.");
                        break;
                    };
                },

                // a Response message is received from the channel
                // this is expected to be sent by the p2p client
                request_msg_opt = self.request_rx.recv() => {
                    if let Some((peer_id, data, channel)) = request_msg_opt {
                        if let Err(e) = self.handle_request((peer_id, data, channel)).await {
                            log::error!("Error handling request: {:?}", e);
                        }
                    } else {
                        log::error!("request_rx channel closed unexpectedly.");
                        break;
                    };
                },

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
    pub async fn shutdown(&mut self) -> Result<()> {
        log::debug!("Sending shutdown command to p2p client.");
        self.p2p.shutdown().await?;

        log::debug!("Closing message channel.");
        self.message_rx.close();

        log::debug!("Closing publish channel.");
        self.publish_rx.close();

        Ok(())
    }
}
