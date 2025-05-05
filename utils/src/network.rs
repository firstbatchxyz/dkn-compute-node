use crate::SemanticVersion;

/// Network type, either mainnet or testnet.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum DriaNetwork {
    Mainnet,
    #[default]
    Testnet,
}

impl From<&str> for DriaNetwork {
    fn from(s: &str) -> Self {
        match s {
            "mainnet" => DriaNetwork::Mainnet,
            "testnet" => DriaNetwork::Testnet,
            _ => Default::default(),
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
    pub fn protocol_name(&self) -> &str {
        match self {
            DriaNetwork::Mainnet => "dria",
            DriaNetwork::Testnet => "dria-test",
        }
    }

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
