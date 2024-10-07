pub mod crypto;
pub mod filter;

mod message;
pub use message::DKNMessage;

mod available_nodes;
pub use available_nodes::AvailableNodes;

use dkn_p2p::libp2p::{multiaddr::Protocol, Multiaddr};
use port_check::is_port_reachable;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    time::{Duration, SystemTime},
};

/// Returns the current time in nanoseconds since the Unix epoch.
///
/// If a `SystemTimeError` occurs, will return 0 just to keep things running.
#[inline]
pub fn get_current_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|e| {
            log::error!("Error getting current time: {}", e);
            Duration::new(0, 0)
        })
        .as_nanos()
}

/// Checks if a given address is already in use locally.
/// This is mostly used to see if the P2P address is already in use.
///
/// Simply tries to connect with TCP to the given address.
#[inline]
pub fn address_in_use(addr: &Multiaddr) -> bool {
    addr.iter()
        // find the port within our multiaddr
        .find_map(|p| {
            if let Protocol::Tcp(port) = p {
                Some(port)
            } else {
                None
            }

            // }
        })
        // check if its reachable or not
        .map(|port| is_port_reachable(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)))
        .unwrap_or_else(|| {
            log::error!(
                "Could not find any TCP port in the given address: {:?}",
                addr
            );
            false
        })
}
