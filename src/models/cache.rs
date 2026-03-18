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

    /// Check if a model's mmproj GGUF is already present in our cache.
    /// Uses model-prefixed filename to avoid collisions between models
    /// that share the same mmproj filename (e.g. multiple Qwen models
    /// all using "mmproj-BF16.gguf" from different repos).
    pub fn get_mmproj_path(&self, spec: &ModelSpec) -> Option<PathBuf> {
        let file = spec.hf_mmproj_file.as_ref()?;
        let prefixed = format!("{}_{}", spec.name.replace(':', "-"), file);
        let path = self.cache_dir.join(&prefixed);
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
            // On non-unix (Windows) we copy files, so verify the copy isn't truncated.
            // A previous interrupted copy could leave a 0-byte or partial file.
            #[cfg(not(unix))]
            {
                let src_len = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);
                let dst_len = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                if src_len > 0 && dst_len != src_len {
                    std::fs::remove_file(&dest)?;
                } else {
                    return Ok(dest);
                }
            }
            #[cfg(unix)]
            return Ok(dest);
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(source, &dest)?;

        #[cfg(not(unix))]
        std::fs::copy(source, &dest)?;

        Ok(dest)
    }

    /// Create a symlink from our cache dir to the hf-hub cached mmproj file.
    /// Uses model-prefixed filename to avoid collisions.
    pub fn link_mmproj(&self, spec: &ModelSpec, source: &Path) -> Result<PathBuf, NodeError> {
        let file = spec
            .hf_mmproj_file
            .as_ref()
            .ok_or_else(|| NodeError::Model("no mmproj file specified".into()))?;
        let prefixed = format!("{}_{}", spec.name.replace(':', "-"), file);
        let dest = self.cache_dir.join(&prefixed);
        if dest.exists() {
            #[cfg(not(unix))]
            {
                let src_len = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);
                let dst_len = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                if src_len > 0 && dst_len != src_len {
                    std::fs::remove_file(&dest)?;
                } else {
                    return Ok(dest);
                }
            }
            #[cfg(unix)]
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
            model_type: dkn_protocol::ModelType::Text,
            hf_mmproj_file: None,
        };

        // Not present initially
        assert!(cache.get_local_path(&spec).is_none());

        // Create the file
        std::fs::write(dir.join("model.gguf"), b"fake").unwrap();
        assert!(cache.get_local_path(&spec).is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmproj_cache_path() {
        let dir = std::env::temp_dir().join("dria-cache-test-mmproj");
        let cache = ModelCache::new(dir.clone()).unwrap();

        let spec_no_mmproj = ModelSpec {
            name: "text:1b".into(),
            hf_repo: "test/repo".into(),
            hf_file: "model.gguf".into(),
            sha256: None,
            model_type: dkn_protocol::ModelType::Text,
            hf_mmproj_file: None,
        };
        assert!(cache.get_mmproj_path(&spec_no_mmproj).is_none());

        let spec_with_mmproj = ModelSpec {
            name: "vl:1b".into(),
            hf_repo: "test/repo".into(),
            hf_file: "model.gguf".into(),
            sha256: None,
            model_type: dkn_protocol::ModelType::Vision,
            hf_mmproj_file: Some("mmproj.gguf".into()),
        };

        // Not present initially
        assert!(cache.get_mmproj_path(&spec_with_mmproj).is_none());

        // Create the prefixed mmproj file
        std::fs::write(dir.join("vl-1b_mmproj.gguf"), b"fake").unwrap();
        assert!(cache.get_mmproj_path(&spec_with_mmproj).is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmproj_no_collision() {
        let dir = std::env::temp_dir().join("dria-cache-test-mmproj-collision");
        let cache = ModelCache::new(dir.clone()).unwrap();

        let spec_a = ModelSpec {
            name: "qwen3.5:0.8b".into(),
            hf_repo: "unsloth/Qwen3.5-0.8B-GGUF".into(),
            hf_file: "model-a.gguf".into(),
            sha256: None,
            model_type: dkn_protocol::ModelType::Vision,
            hf_mmproj_file: Some("mmproj-BF16.gguf".into()),
        };

        let spec_b = ModelSpec {
            name: "qwen3.5:27b".into(),
            hf_repo: "unsloth/Qwen3.5-27B-GGUF".into(),
            hf_file: "model-b.gguf".into(),
            sha256: None,
            model_type: dkn_protocol::ModelType::Vision,
            hf_mmproj_file: Some("mmproj-BF16.gguf".into()),
        };

        // Create separate source files
        std::fs::write(dir.join("mmproj_a.gguf"), b"small_model").unwrap();
        std::fs::write(dir.join("mmproj_b.gguf"), b"large_model").unwrap();

        let path_a = cache.link_mmproj(&spec_a, &dir.join("mmproj_a.gguf")).unwrap();
        let path_b = cache.link_mmproj(&spec_b, &dir.join("mmproj_b.gguf")).unwrap();

        // Paths must be different
        assert_ne!(path_a, path_b);
        assert!(path_a.exists());
        assert!(path_b.exists());

        // Content must be independent
        assert_eq!(std::fs::read(&path_a).unwrap(), b"small_model");
        assert_eq!(std::fs::read(&path_b).unwrap(), b"large_model");

        std::fs::remove_dir_all(&dir).ok();
    }
}
