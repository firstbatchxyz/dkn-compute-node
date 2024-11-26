use libp2p::StreamProtocol;
use std::env;

#[derive(Clone, Debug)]
pub struct DriaP2PProtocol {
    /// Main protocol name, e.g. `dria`.
    pub name: String,
    /// Version of the protocol, e.g. `0.2`.
    /// By default, this is set to the current `major.minor` version of the crate.
    pub version: String,
    /// Identity protocol string to be used for the Identity behaviour.
    ///
    /// This is usually `{name}/{version}`.
    pub identity: String,
    /// Kademlia protocol, must match with other peers in the network.
    ///
    /// This is usually `/{name}/kad/{version}`, notice the `/` at the start
    /// which is mandatory for a `StreamProtocol`.
    ///
    pub kademlia: StreamProtocol,
}

impl std::fmt::Display for DriaP2PProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.identity)
    }
}

impl Default for DriaP2PProtocol {
    /// Creates a new instance of the protocol with the default name `dria`.
    fn default() -> Self {
        Self::new_major_minor("dria")
    }
}

impl DriaP2PProtocol {
    /// Creates a new instance of the protocol with the given `name` and `version`.
    pub fn new(name: &str, version: &str) -> Self {
        let identity = format!("{}/{}", name, version);
        let kademlia = format!("/{}/kad/{}", name, version);

        Self {
            name: name.to_string(),
            version: version.to_string(),
            identity,
            kademlia: StreamProtocol::try_from_owned(kademlia).unwrap(), // guaranteed to unwrap
        }
    }

    /// Creates a new instance of the protocol with the given `name` and the current version as per Cargo.toml.
    /// The verison is represented with `major.minor` version numbers.
    pub fn new_major_minor(name: &str) -> Self {
        const VERSION: &str = concat!(
            env!("CARGO_PKG_VERSION_MAJOR"),
            ".",
            env!("CARGO_PKG_VERSION_MINOR")
        );

        Self::new(name, VERSION)
    }

    /// Returns the identity protocol, e.g. `dria/0.2`.
    pub fn identity(&self) -> String {
        self.identity.clone()
    }

    /// Returns the kademlia protocol, e.g. `/dria/kad/0.2`.
    pub fn kademlia(&self) -> StreamProtocol {
        self.kademlia.clone()
    }

    /// Returns `true` if the given protocol has a matching prefix with out Kademlia protocol.
    /// Otherwise, returns `false`.
    pub fn is_common_kademlia(&self, protocol: &StreamProtocol) -> bool {
        let kad_prefix = format!("/{}/kad/", self.name);
        protocol.to_string().starts_with(&kad_prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::StreamProtocol;

    #[test]
    fn test_new() {
        let protocol = DriaP2PProtocol::new("test", "1.0");
        assert_eq!(protocol.name, "test");
        assert_eq!(protocol.version, "1.0");
        assert_eq!(protocol.identity, "test/1.0");
        assert_eq!(protocol.kademlia.to_string(), "/test/kad/1.0");
    }

    #[test]
    fn test_new_major_minor() {
        let protocol = DriaP2PProtocol::new_major_minor("test");
        assert_eq!(protocol.name, "test");
        assert_eq!(
            protocol.version,
            concat!(
                env!("CARGO_PKG_VERSION_MAJOR"),
                ".",
                env!("CARGO_PKG_VERSION_MINOR")
            )
        );
        assert_eq!(protocol.identity, format!("test/{}", protocol.version));
        assert_eq!(
            protocol.kademlia.to_string(),
            format!("/test/kad/{}", protocol.version)
        );
    }

    #[test]
    fn test_is_common_kademlia() {
        let protocol = DriaP2PProtocol::new("test", "1.0");
        let matching_protocol = StreamProtocol::new("/test/kad/1.0");
        let non_matching_protocol = StreamProtocol::new("/other/kad/1.0");

        assert!(protocol.is_common_kademlia(&matching_protocol));
        assert!(!protocol.is_common_kademlia(&non_matching_protocol));
    }
}
