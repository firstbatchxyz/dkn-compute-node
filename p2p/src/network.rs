/// Network type.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum DriaNetworkType {
    #[default]
    Community,
    Pro,
    Test,
}

impl From<&str> for DriaNetworkType {
    fn from(s: &str) -> Self {
        match s {
            "community" => DriaNetworkType::Community,
            "pro" => DriaNetworkType::Pro,
            "test" => DriaNetworkType::Test,
            _ => Default::default(),
        }
    }
}

impl std::fmt::Display for DriaNetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriaNetworkType::Community => write!(f, "community"),
            DriaNetworkType::Pro => write!(f, "pro"),
            DriaNetworkType::Test => write!(f, "test"),
        }
    }
}

impl DriaNetworkType {
    /// Returns the protocol name.
    pub fn protocol_name(&self) -> &str {
        match self {
            DriaNetworkType::Community => "dria",
            DriaNetworkType::Pro => "dria-sdk",
            DriaNetworkType::Test => "dria-test",
        }
    }
}
