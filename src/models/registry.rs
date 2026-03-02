use std::collections::HashMap;

/// Specification for a model: shortname mapped to HuggingFace GGUF location.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Short name used by users (e.g. "gemma3:4b")
    pub name: String,
    /// HuggingFace repository (e.g. "bartowski/gemma-3-4b-it-GGUF")
    pub hf_repo: String,
    /// Filename within the repo (e.g. "gemma-3-4b-it-Q4_K_M.gguf")
    pub hf_file: String,
    /// Expected SHA-256 hex digest for verification (None = skip verification)
    pub sha256: Option<String>,
    /// Chat template identifier (e.g. "gemma", "llama3", "chatml")
    pub chat_template: Option<String>,
}

/// Build the default model registry with all 9 supported models.
pub fn default_registry() -> HashMap<String, ModelSpec> {
    let entries = vec![
        ModelSpec {
            name: "gemma3:4b".into(),
            hf_repo: "bartowski/google_gemma-3-4b-it-GGUF".into(),
            hf_file: "google_gemma-3-4b-it-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("gemma".into()),
        },
        ModelSpec {
            name: "gemma3:12b".into(),
            hf_repo: "bartowski/google_gemma-3-12b-it-GGUF".into(),
            hf_file: "google_gemma-3-12b-it-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("gemma".into()),
        },
        ModelSpec {
            name: "gemma3:27b".into(),
            hf_repo: "bartowski/google_gemma-3-27b-it-GGUF".into(),
            hf_file: "google_gemma-3-27b-it-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("gemma".into()),
        },
        ModelSpec {
            name: "llama3.1:8b".into(),
            hf_repo: "bartowski/Meta-Llama-3.1-8B-Instruct-GGUF".into(),
            hf_file: "Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("llama3".into()),
        },
        ModelSpec {
            name: "llama3.2:1b".into(),
            hf_repo: "bartowski/Llama-3.2-1B-Instruct-GGUF".into(),
            hf_file: "Llama-3.2-1B-Instruct-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("llama3".into()),
        },
        ModelSpec {
            name: "llama3.3:70b".into(),
            hf_repo: "bartowski/Llama-3.3-70B-Instruct-GGUF".into(),
            hf_file: "Llama-3.3-70B-Instruct-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("llama3".into()),
        },
        ModelSpec {
            name: "mistral-nemo:12b".into(),
            hf_repo: "bartowski/Mistral-Nemo-Instruct-2407-GGUF".into(),
            hf_file: "Mistral-Nemo-Instruct-2407-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
        },
        ModelSpec {
            name: "qwen3:8b".into(),
            hf_repo: "bartowski/Qwen3-8B-GGUF".into(),
            hf_file: "Qwen3-8B-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
        },
        ModelSpec {
            name: "qwen3:32b".into(),
            hf_repo: "bartowski/Qwen3-32B-GGUF".into(),
            hf_file: "Qwen3-32B-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
        },
    ];

    entries.into_iter().map(|s| (s.name.clone(), s)).collect()
}

/// Resolve a user-provided model name to a ModelSpec from the registry.
pub fn resolve_model(name: &str, registry: &HashMap<String, ModelSpec>) -> Option<ModelSpec> {
    registry.get(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_all_models() {
        let reg = default_registry();
        let expected = [
            "gemma3:4b",
            "gemma3:12b",
            "gemma3:27b",
            "llama3.1:8b",
            "llama3.2:1b",
            "llama3.3:70b",
            "mistral-nemo:12b",
            "qwen3:8b",
            "qwen3:32b",
        ];
        for name in &expected {
            assert!(reg.contains_key(*name), "missing model: {name}");
        }
        assert_eq!(reg.len(), 9);
    }

    #[test]
    fn test_resolve_known_model() {
        let reg = default_registry();
        let spec = resolve_model("gemma3:4b", &reg).expect("should resolve");
        assert_eq!(spec.name, "gemma3:4b");
        assert!(spec.hf_repo.contains("gemma"));
        assert!(spec.hf_file.ends_with(".gguf"));
    }

    #[test]
    fn test_resolve_unknown_model() {
        let reg = default_registry();
        assert!(resolve_model("nonexistent:1b", &reg).is_none());
    }
}
