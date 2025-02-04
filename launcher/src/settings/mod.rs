use enum_iterator::Sequence;

mod models;
pub use models::edit_models;

mod apikey;
pub use apikey::edit_api_keys;

mod wallet;
pub use wallet::edit_wallet;

/// Compute node setting commands.
#[derive(Debug, Clone, Sequence)]
pub enum Settings {
    /// Configure your wallet (secret key).
    Wallet,
    /// Configure the selected models.
    Models,
    /// Configure your API Keys.
    ApiKeys,
    /// Quit settings menu.
    SaveExit,
}

impl Settings {
    #[inline]
    pub fn all() -> Vec<Self> {
        enum_iterator::all::<Self>().collect()
    }
}

impl std::fmt::Display for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wallet => write!(f, "Wallet"),
            Self::Models => write!(f, "Models"),
            Self::ApiKeys => write!(f, "API Keys"),
            Self::SaveExit => write!(f, "Save & Exit"),
        }
    }
}
