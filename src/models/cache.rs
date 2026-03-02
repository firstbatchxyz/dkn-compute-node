use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::NodeError;
use crate::models::registry::ModelSpec;

/// Manages local model file cache.
#[derive(Clone)]
pub struct ModelCache {
    pub cache_dir: PathBuf,
}

impl ModelCache {
    /// Create a new cache backed by the given directory.
    pub fn new(cache_dir: PathBuf) -> Result<Self, NodeError> {
        std::fs::create_dir_all(&cache_dir)?;
        Ok(ModelCache { cache_dir })
    }

    /// Check if a model's GGUF is already present in our cache.
    pub fn get_local_path(&self, spec: &ModelSpec) -> Option<PathBuf> {
        let path = self.cache_dir.join(&spec.hf_file);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Verify a file's SHA-256 against an expected hex digest.
    /// Returns Ok(true) if matches, Ok(false) if mismatch, Err on I/O failure.
    pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<bool, NodeError> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let actual = hex::encode(hasher.finalize());
        Ok(actual == expected_hex.to_lowercase())
    }

    /// Create a symlink from our cache dir to the hf-hub cached file.
    /// This avoids duplicating multi-GB files on disk.
    pub fn link_model(&self, spec: &ModelSpec, source: &Path) -> Result<PathBuf, NodeError> {
        let dest = self.cache_dir.join(&spec.hf_file);
        if dest.exists() {
            // Already linked or copied
            return Ok(dest);
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(source, &dest)?;

        #[cfg(not(unix))]
        std::fs::copy(source, &dest)?;

        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_verify_sha256() {
        let dir = std::env::temp_dir().join("dria-cache-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.bin");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello world").unwrap();
        drop(f);

        // SHA-256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(ModelCache::verify_sha256(&file_path, expected).unwrap());
        assert!(!ModelCache::verify_sha256(&file_path, "0000000000000000000000000000000000000000000000000000000000000000").unwrap());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_cache_local_path() {
        let dir = std::env::temp_dir().join("dria-cache-test-2");
        let cache = ModelCache::new(dir.clone()).unwrap();

        let spec = ModelSpec {
            name: "test:1b".into(),
            hf_repo: "test/repo".into(),
            hf_file: "model.gguf".into(),
            sha256: None,
            chat_template: None,
            model_type: dkn_protocol::ModelType::Text,
        };

        // Not present initially
        assert!(cache.get_local_path(&spec).is_none());

        // Create the file
        std::fs::write(dir.join("model.gguf"), b"fake").unwrap();
        assert!(cache.get_local_path(&spec).is_some());

        std::fs::remove_dir_all(&dir).ok();
    }
}
