use std::time::Duration;
use tokio::time::Instant;

use crate::{refresh_dria_nodes, DriaComputeNode, DRIA_COMPUTE_NODE_VERSION};

/// Number of seconds such that if the last ping is older than this, the node is considered unreachable.
const PING_LIVENESS_SECS: u64 = 150;

impl DriaComputeNode {
    /// Returns the task count within the channels, `single` and `batch`.
    #[inline(always)]
    pub fn get_pending_task_count(&self) -> [usize; 2] {
        [
            self.pending_tasks_single.len(),
            self.pending_tasks_batch.len(),
        ]
    }

    /// Peer refresh simply reports the peer count to the user.
    pub(crate) async fn handle_diagnostic_refresh(&self) {
        let mut diagnostics = vec![format!("Diagnostics (v{}):", DRIA_COMPUTE_NODE_VERSION)];

        // print peer counts
        match self.p2p.peer_counts().await {
            Ok((mesh, all)) => {
                diagnostics.push(format!("Peer Count (mesh/all): {} / {}", mesh, all))
            }
            Err(e) => log::error!("Error getting peer counts: {:?}", e),
        }

        // print tasks count
        let [single, batch] = self.get_pending_task_count();
        diagnostics.push(format!(
            "Pending Tasks (single/batch): {} / {}",
            single, batch
        ));

        // completed tasks count is printed as well in debug
        if log::log_enabled!(log::Level::Debug) {
            diagnostics.push(format!(
                "Completed Tasks (single/batch): {} / {}",
                self.completed_tasks_single, self.completed_tasks_batch
            ));
        }

        // print peer id and address
        diagnostics.push(format!("Peer ID: {}", self.config.peer_id));
        diagnostics.push(format!("Address: 0x{}", self.config.address));

        // print models
        diagnostics.push(format!(
            "Models: {}",
            self.config
                .workflows
                .models
                .iter()
                .map(|(p, m)| format!("{}/{}", p, m))
                .collect::<Vec<String>>()
                .join(", ")
        ));

        log::info!("{}", diagnostics.join("\n  "));

        // check liveness of the node w.r.t last ping-pong time
        if self.last_pinged_at < Instant::now() - Duration::from_secs(PING_LIVENESS_SECS) {
            log::error!(
                "Node has not received any pings for at least {} seconds & it may be unreachable!\nPlease restart your node!",
                PING_LIVENESS_SECS
            );
        }

        // added rpc nodes check, sometimes this happens when API is down / bugs for some reason
        if self.dria_nodes.rpc_peerids.is_empty() {
            log::error!("No RPC peerids were found to be available, please restart your node!",);
        }
    }

    /// Updates the local list of available nodes by refreshing it.
    /// Dials the RPC nodes again for better connectivity.
    pub(crate) async fn handle_available_nodes_refresh(&mut self) {
        log::info!("Refreshing available Dria nodes.");

        // refresh available nodes
        if let Err(e) = refresh_dria_nodes(&mut self.dria_nodes).await {
            log::error!("Error refreshing available nodes: {:?}", e);
        };

        // dial all rpc nodes
        for rpc_addr in self.dria_nodes.rpc_nodes.iter() {
            log::info!("Dialling RPC node: {}", rpc_addr);

            let fut = self.p2p.dial(rpc_addr.clone());
            match tokio::time::timeout(Duration::from_secs(10), fut).await {
                Err(timeout) => {
                    log::error!("Timeout dialling RPC node: {:?}", timeout);
                }
                Ok(res) => match res {
                    Err(e) => {
                        log::warn!("Error dialling RPC node: {:?}", e);
                    }
                    Ok(_) => {
                        log::info!("Successfully dialled RPC node: {}", rpc_addr);
                    }
                },
            };
        }

        log::info!("Finished refreshing!");
    }
}
