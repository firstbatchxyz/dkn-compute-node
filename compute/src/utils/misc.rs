use dkn_p2p::libp2p::{multiaddr::Protocol, Multiaddr};
use port_check::is_port_reachable;
use std::net::{Ipv4Addr, SocketAddrV4};

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
                "could not find any TCP port in the given address: {:?}",
                addr
            );
            false
        })
}
