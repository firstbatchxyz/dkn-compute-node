use std::path::PathBuf;

use hf_hub::api::tokio::ApiBuilder;

use crate::error::NodeError;
use crate::models::registry::ModelSpec;

/// Downloads GGUF models from HuggingFace using the `hf-hub` crate.
pub struct ModelDownloader;

impl ModelDownloader {
    /// Download a model's GGUF file from HuggingFace.
    ///
    /// Uses hf-hub's built-in cache (defaults to `~/.cache/huggingface/`)
    /// and supports automatic resume of interrupted downloads.
    ///
    /// Returns the local path to the downloaded file.
    pub async fn download(spec: &ModelSpec) -> Result<PathBuf, NodeError> {
        let api = ApiBuilder::new()
            .with_progress(true)
            .build()
            .map_err(|e| NodeError::Model(format!("failed to create HF API client: {e}")))?;

        let repo = api.model(spec.hf_repo.clone());

        tracing::info!(
            model = %spec.name,
            repo = %spec.hf_repo,
            file = %spec.hf_file,
            "downloading model from HuggingFace"
        );

        let path = repo
            .get(&spec.hf_file)
            .await
            .map_err(|e| NodeError::Model(format!("failed to download {}: {e}", spec.name)))?;

        tracing::info!(
            model = %spec.name,
            path = %path.display(),
            "model download complete"
        );

        Ok(path)
    }

    /// Download the multimodal projector GGUF from HuggingFace.
    ///
    /// Returns the local path to the downloaded mmproj file.
    pub async fn download_mmproj(spec: &ModelSpec) -> Result<PathBuf, NodeError> {
        let mmproj_file = spec
            .hf_mmproj_file
            .as_ref()
            .ok_or_else(|| NodeError::Model("no mmproj file specified".into()))?;

        let api = ApiBuilder::new()
            .with_progress(true)
            .build()
            .map_err(|e| NodeError::Model(format!("failed to create HF API client: {e}")))?;

        let repo = api.model(spec.hf_repo.clone());

        tracing::info!(
            model = %spec.name,
            repo = %spec.hf_repo,
            file = %mmproj_file,
            "downloading mmproj from HuggingFace"
        );

        let path = repo.get(mmproj_file).await.map_err(|e| {
            NodeError::Model(format!(
                "failed to download mmproj for {}: {e}",
                spec.name
            ))
        })?;

        tracing::info!(
            model = %spec.name,
            path = %path.display(),
            "mmproj download complete"
        );

        Ok(path)
    }
}
