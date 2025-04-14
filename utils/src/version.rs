use std::str::FromStr;

/// A tiny utility for semantic versioning.
/// This is a simple struct that holds the major, minor, and patch version numbers.
///
/// Implements a Display trait that serializes to `{major}.{minor}.{patch}`.
#[derive(serde::Serialize, serde::Deserialize, Debug, Default, Clone, PartialEq, Eq, Copy)]
pub struct SemanticVersion {
    /// Major version number.
    pub major: u32,
    /// Minor version number.
    pub minor: u32,
    /// Patch version number.
    pub patch: u32,
}

impl FromStr for SemanticVersion {
    type Err = String;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let parts: Vec<u32> = version.split('.').filter_map(|s| s.parse().ok()).collect();

        if parts.len() != 3 {
            Err("Invalid version format".to_string())
        } else {
            Ok(SemanticVersion {
                major: parts[0],
                minor: parts[1],
                patch: parts[2],
            })
        }
    }
}

impl SemanticVersion {
    /// Checks if the current version is compatible with the given version.
    /// Compatibility is defined as:
    /// - Major and minor versions must match exactly.
    /// - Patch version must be greater than or equal to the given version.
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch >= other.patch
    }

    pub fn with_major(mut self, major: u32) -> Self {
        self.major = major;
        self
    }

    pub fn with_minor(mut self, minor: u32) -> Self {
        self.minor = minor;
        self
    }

    pub fn with_patch(mut self, patch: u32) -> Self {
        self.patch = patch;
        self
    }

    /// Parses the Crate version field into `SemanticVersion`.
    ///
    /// Will panic if for any reason the version format is wrong.
    #[inline]
    pub fn from_crate_version() -> Self {
        env!("CARGO_PKG_VERSION").parse().unwrap()
    }
}

impl std::fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
