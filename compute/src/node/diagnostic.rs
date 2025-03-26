use colored::Colorize;
use dkn_p2p::libp2p::multiaddr::Protocol;
use std::time::Duration;

use crate::{refresh_dria_nodes, utils::get_points, DriaComputeNode, DRIA_COMPUTE_NODE_VERSION};

/// Number of seconds such that if the last heartbeat ACK is older than this, the node is considered unreachable.
const HEARTBEAT_LIVENESS_SECS: Duration = Duration::from_secs(150);

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

        // print steps
        if let Ok(steps) = get_points(&self.config.address).await {
            let earned = steps.score - self.initial_steps;
            diagnostics.push(format!(
                "$DRIA Points: {} total, {} earned in this run, within top {}%",
                steps.score, earned, steps.percentile
            ));
        }

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

        // if we have not received pings for a while, we are considered offline
        let is_offline = chrono::Utc::now() > self.last_heartbeat_at + HEARTBEAT_LIVENESS_SECS;

        // if we have not yet received a heartbeat response, we are still connecting
        if self.num_heartbeats == 0 {
            // if we didnt have any pings, we might still be connecting
            diagnostics.push(format!("Node Status: {}", "CONNECTING".yellow()));
        } else {
            diagnostics.push(format!(
                "Node Status: {}",
                if is_offline {
                    "OFFLINE".red()
                } else {
                    "ONLINE".green()
                }
            ));
        }

        log::info!("{}", diagnostics.join("\n  "));

        // if offline, print this error message as well
        if is_offline {
            log::error!(
                "Node has not received any pings for at least {} seconds & it may be unreachable!\nPlease restart your node!",
                HEARTBEAT_LIVENESS_SECS.as_secs()
            );
        }

        // added rpc nodes check, sometimes this happens when API is down / bugs for some reason
        if self.dria_nodes.rpc_peerids.is_empty() {
            log::error!("No RPC peer IDs were found to be available, please restart your node!",);
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
        for addr in self.dria_nodes.rpc_nodes.iter() {
            log::info!("Dialling RPC node: {}", addr);

            // get peer id from rpc address
            if let Some(peer_id) = addr.iter().find_map(|p| match p {
                Protocol::P2p(peer_id) => Some(peer_id),
                _ => None,
            }) {
                let fut = self.p2p.dial(peer_id, addr.clone());
                match tokio::time::timeout(Duration::from_secs(10), fut).await {
                    Err(timeout) => {
                        log::error!("Timeout dialling RPC node: {:?}", timeout);
                    }
                    Ok(res) => match res {
                        Err(e) => {
                            log::warn!("Error dialling RPC node: {:?}", e);
                        }
                        Ok(_) => {
                            log::info!("Successfully dialled RPC node: {}", addr);
                        }
                    },
                };
            } else {
                log::warn!("Missing peerID in address: {}", addr);
            }
        }

        log::info!("Finished refreshing!");
    }
}
