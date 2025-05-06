use crate::SemanticVersion;

/// Network type, either mainnet or testnet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriaNetwork {
    Mainnet,
    Testnet,
}

impl TryFrom<&str> for DriaNetwork {
    type Error = ();

    /// Converts a string to a `DriaNetwork`, using the same name as in:
    ///
    /// - "mainnet" for `DriaNetwork::Mainnet`
    /// - "testnet" for `DriaNetwork::Testnet`
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "mainnet" => Ok(DriaNetwork::Mainnet),
            "testnet" => Ok(DriaNetwork::Testnet),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for DriaNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriaNetwork::Mainnet => write!(f, "mainnet"),
            DriaNetwork::Testnet => write!(f, "testnet"),
        }
    }
}

impl DriaNetwork {
    /// Returns the protocol name for the given network, which can be used by
    /// libp2p `identify` protocol.
    pub fn protocol_name(&self) -> &str {
        match self {
            DriaNetwork::Mainnet => "dria",
            DriaNetwork::Testnet => "dria-test",
        }
    }

    /// Returns the discovery URL for the given version, where the
    /// major.minor version is appended to the URL as a path variable.
    pub fn discovery_url(&self, version: &SemanticVersion) -> String {
        let base_url = match self {
            DriaNetwork::Mainnet => "https://mainnet.dkn.dria.co/discovery/v0/available-nodes",
            DriaNetwork::Testnet => "https://testnet.dkn.dria.co/discovery/v0/available-nodes",
        };

        format!("{}/{}", base_url, version.as_major_minor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dria_network() {
        let mainnet = DriaNetwork::Mainnet;
        let testnet = DriaNetwork::Testnet;
        let version = SemanticVersion {
            major: 1,
            minor: 0,
            patch: 42,
        };

        assert_eq!(mainnet.to_string(), "mainnet");
        assert_eq!(testnet.to_string(), "testnet");

        assert_eq!(mainnet.protocol_name(), "dria");
        assert_eq!(testnet.protocol_name(), "dria-test");

        assert_eq!(
            mainnet.discovery_url(&version),
            "https://mainnet.dkn.dria.co/discovery/v0/available-nodes/1.0"
        );
        assert_eq!(
            testnet.discovery_url(&version),
            "https://testnet.dkn.dria.co/discovery/v0/available-nodes/1.0"
        );
    }
}
