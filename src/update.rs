use std::time::Duration;

use semver::Version;

use crate::error::NodeError;

const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/firstbatchxyz/dkn-compute-node/releases/latest";

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[allow(dead_code)]
    assets: Vec<GitHubAsset>,
    prerelease: bool,
}

#[derive(serde::Deserialize)]
struct GitHubAsset {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    browser_download_url: String,
}

pub enum UpdateAction {
    UpToDate,
    Warn(String),
    Force(String),
}

/// Determine the correct release asset name for this platform.
fn asset_name() -> Result<&'static str, NodeError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "x86_64") => Ok("dria-node-macOS-amd64"),
        ("macos", "aarch64") => Ok("dria-node-macOS-arm64"),
        ("linux", "x86_64") => Ok("dria-node-linux-amd64"),
        ("linux", "aarch64") => Ok("dria-node-linux-arm64"),
        ("windows", "x86_64") => Ok("dria-node-windows-amd64.exe"),
        (os, arch) => Err(NodeError::Update(format!(
            "unsupported platform: {os}/{arch}"
        ))),
    }
}

/// Classify the update action based on semver comparison.
fn classify_update(current: &Version, latest: &Version) -> UpdateAction {
    if latest <= current {
        return UpdateAction::UpToDate;
    }
    if current.major == latest.major && current.minor == latest.minor {
        UpdateAction::Warn(latest.to_string())
    } else {
        UpdateAction::Force(latest.to_string())
    }
}

/// Check GitHub for the latest release and compare with current version.
pub async fn check_for_update() -> Result<UpdateAction, NodeError> {
    let current_version = env!("CARGO_PKG_VERSION");
    let current = Version::parse(current_version)
        .map_err(|e| NodeError::Update(format!("invalid current version: {e}")))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .user_agent(format!("dria-node/{current_version}"))
        .build()
        .map_err(|e| NodeError::Update(format!("http client error: {e}")))?;

    let release: GitHubRelease = client
        .get(GITHUB_RELEASES_URL)
        .send()
        .await
        .map_err(|e| NodeError::Update(format!("failed to fetch release info: {e}")))?
        .json()
        .await
        .map_err(|e| NodeError::Update(format!("failed to parse release info: {e}")))?;

    if release.prerelease {
        return Ok(UpdateAction::UpToDate);
    }

    let tag = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);
    let latest = Version::parse(tag)
        .map_err(|e| NodeError::Update(format!("invalid release version '{tag}': {e}")))?;

    Ok(classify_update(&current, &latest))
}

/// Download the update binary and replace the current executable.
pub async fn perform_update(version: &str) -> Result<(), NodeError> {
    let asset = asset_name()?;

    let url = format!(
        "https://github.com/firstbatchxyz/dkn-compute-node/releases/download/v{version}/{asset}"
    );

    tracing::info!(%url, "downloading update");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(format!("dria-node/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| NodeError::Update(format!("http client error: {e}")))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| NodeError::Update(format!("download failed: {e}")))?;

    if !response.status().is_success() {
        return Err(NodeError::Update(format!(
            "download failed with status: {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| NodeError::Update(format!("failed to read download: {e}")))?;

    // Write to a temp file
    let mut tmp = tempfile::NamedTempFile::new()
        .map_err(|e| NodeError::Update(format!("failed to create temp file: {e}")))?;

    std::io::Write::write_all(&mut tmp, &bytes)
        .map_err(|e| NodeError::Update(format!("failed to write temp file: {e}")))?;

    // Set executable permission on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755))
            .map_err(|e| NodeError::Update(format!("failed to set permissions: {e}")))?;
    }

    // Atomic self-replace
    self_replace::self_replace(tmp.path())
        .map_err(|e| NodeError::Update(format!("self-replace failed: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_same_version_is_up_to_date() {
        let current = Version::new(0, 7, 2);
        let latest = Version::new(0, 7, 2);
        assert!(matches!(classify_update(&current, &latest), UpdateAction::UpToDate));
    }

    #[test]
    fn classify_patch_bump_is_warn() {
        let current = Version::new(0, 7, 2);
        let latest = Version::new(0, 7, 3);
        assert!(matches!(classify_update(&current, &latest), UpdateAction::Warn(v) if v == "0.7.3"));
    }

    #[test]
    fn classify_minor_bump_is_force() {
        let current = Version::new(0, 7, 2);
        let latest = Version::new(0, 8, 0);
        assert!(matches!(classify_update(&current, &latest), UpdateAction::Force(v) if v == "0.8.0"));
    }

    #[test]
    fn classify_major_bump_is_force() {
        let current = Version::new(0, 7, 2);
        let latest = Version::new(1, 0, 0);
        assert!(matches!(classify_update(&current, &latest), UpdateAction::Force(v) if v == "1.0.0"));
    }

    #[test]
    fn classify_older_release_is_up_to_date() {
        let current = Version::new(0, 8, 0);
        let latest = Version::new(0, 7, 2);
        assert!(matches!(classify_update(&current, &latest), UpdateAction::UpToDate));
    }

    #[test]
    fn asset_name_returns_value_for_current_platform() {
        // Should not error on any CI/dev platform we support
        let name = asset_name().unwrap();
        assert!(name.starts_with("dria-node-"));
    }

    #[test]
    fn parse_github_release_json() {
        let json = r#"{
            "tag_name": "v0.8.0",
            "prerelease": false,
            "assets": [
                {
                    "name": "dria-node-linux-amd64",
                    "browser_download_url": "https://github.com/firstbatchxyz/dkn-compute-node/releases/download/v0.8.0/dria-node-linux-amd64"
                }
            ]
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v0.8.0");
        assert!(!release.prerelease);
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "dria-node-linux-amd64");
    }
}
