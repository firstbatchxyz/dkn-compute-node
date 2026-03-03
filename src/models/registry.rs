use std::collections::HashMap;

use dkn_protocol::{ModelRegistryEntry, ModelType};

/// Specification for a model: shortname mapped to HuggingFace GGUF location.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Short name used by users (e.g. "lfm2.5:1.2b")
    pub name: String,
    /// HuggingFace repository (e.g. "LiquidAI/LFM2.5-1.2B-Instruct-GGUF")
    pub hf_repo: String,
    /// Filename within the repo (e.g. "LFM2.5-1.2B-Instruct-Q4_K_M.gguf")
    pub hf_file: String,
    /// Expected SHA-256 hex digest for verification (None = skip verification)
    pub sha256: Option<String>,
    /// Chat template identifier (e.g. "gemma", "llama3", "chatml")
    pub chat_template: Option<String>,
    /// Modality this model supports.
    pub model_type: ModelType,
}

/// Build the default model registry with all supported models.
pub fn default_registry() -> HashMap<String, ModelSpec> {
    let entries = vec![
        ModelSpec {
            name: "lfm2.5:1.2b".into(),
            hf_repo: "LiquidAI/LFM2.5-1.2B-Instruct-GGUF".into(),
            hf_file: "LFM2.5-1.2B-Instruct-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
        ModelSpec {
            name: "qwen3.5:35b-a3b".into(),
            hf_repo: "unsloth/Qwen3.5-35B-A3B-GGUF".into(),
            hf_file: "Qwen3.5-35B-A3B-UD-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
        ModelSpec {
            name: "lfm2:24b-a2b".into(),
            hf_repo: "LiquidAI/LFM2-24B-A2B-GGUF".into(),
            hf_file: "LFM2-24B-A2B-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
        ModelSpec {
            name: "lfm2.5-vl:1.6b".into(),
            hf_repo: "LiquidAI/LFM2.5-VL-1.6B-GGUF".into(),
            hf_file: "LFM2.5-VL-1.6B-Q4_0.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Vision,
        },
        ModelSpec {
            name: "lfm2.5-audio:1.5b".into(),
            hf_repo: "LiquidAI/LFM2.5-Audio-1.5B-GGUF".into(),
            hf_file: "LFM2.5-Audio-1.5B-Q4_0.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Audio,
        },
        ModelSpec {
            name: "qwen3.5:27b".into(),
            hf_repo: "unsloth/Qwen3.5-27B-GGUF".into(),
            hf_file: "Qwen3.5-27B-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
        ModelSpec {
            name: "nanbeige:3b".into(),
            hf_repo: "DevQuasar/Nanbeige.Nanbeige4.1-3B-GGUF".into(),
            hf_file: "Nanbeige.Nanbeige4.1-3B.Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
        ModelSpec {
            name: "locooperator:4b".into(),
            hf_repo: "LocoreMind/LocoOperator-4B-GGUF".into(),
            hf_file: "LocoOperator-4B.Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
        ModelSpec {
            name: "qwen3.5:9b".into(),
            hf_repo: "lmstudio-community/Qwen3.5-9B-GGUF".into(),
            hf_file: "Qwen3.5-9B-Q4_K_M.gguf".into(),
            sha256: None,
            chat_template: Some("chatml".into()),
            model_type: ModelType::Text,
        },
    ];

    entries.into_iter().map(|s| (s.name.clone(), s)).collect()
}

impl ModelSpec {
    /// Create a ModelSpec from a router-provided registry entry.
    pub fn from_registry_entry(entry: &ModelRegistryEntry) -> Self {
        ModelSpec {
            name: entry.name.clone(),
            hf_repo: entry.hf_repo.clone(),
            hf_file: entry.hf_file.clone(),
            sha256: None,
            chat_template: entry.chat_template.clone(),
            model_type: entry.model_type,
        }
    }
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
            "lfm2.5:1.2b",
            "qwen3.5:35b-a3b",
            "lfm2:24b-a2b",
            "lfm2.5-vl:1.6b",
            "lfm2.5-audio:1.5b",
            "qwen3.5:27b",
            "nanbeige:3b",
            "locooperator:4b",
            "qwen3.5:9b",
        ];
        for name in &expected {
            assert!(reg.contains_key(*name), "missing model: {name}");
        }
        assert_eq!(reg.len(), 9);
    }

    #[test]
    fn test_resolve_known_model() {
        let reg = default_registry();
        let spec = resolve_model("lfm2.5:1.2b", &reg).expect("should resolve");
        assert_eq!(spec.name, "lfm2.5:1.2b");
        assert!(spec.hf_repo.contains("LFM2.5"));
        assert!(spec.hf_file.ends_with(".gguf"));
        assert_eq!(spec.model_type, ModelType::Text);
    }

    #[test]
    fn test_resolve_unknown_model() {
        let reg = default_registry();
        assert!(resolve_model("nonexistent:1b", &reg).is_none());
    }

    #[test]
    fn test_from_registry_entry() {
        let entry = ModelRegistryEntry {
            name: "test:1b".into(),
            hf_repo: "test/repo".into(),
            hf_file: "model.gguf".into(),
            chat_template: Some("chatml".into()),
            model_type: ModelType::Vision,
        };
        let spec = ModelSpec::from_registry_entry(&entry);
        assert_eq!(spec.name, "test:1b");
        assert_eq!(spec.hf_repo, "test/repo");
        assert_eq!(spec.hf_file, "model.gguf");
        assert!(spec.sha256.is_none());
        assert_eq!(spec.chat_template, Some("chatml".into()));
        assert_eq!(spec.model_type, ModelType::Vision);
    }

    #[test]
    fn test_model_types_correct() {
        let reg = default_registry();
        assert_eq!(reg["lfm2.5-vl:1.6b"].model_type, ModelType::Vision);
        assert_eq!(reg["lfm2.5-audio:1.5b"].model_type, ModelType::Audio);
        assert_eq!(reg["lfm2.5:1.2b"].model_type, ModelType::Text);
        assert_eq!(reg["qwen3.5:27b"].model_type, ModelType::Text);
    }
}
