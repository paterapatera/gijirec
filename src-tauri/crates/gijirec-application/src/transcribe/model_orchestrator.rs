//! Model existence verification and download orchestration.

use std::path::{Path, PathBuf};

use gijirec_domain::transcribe::{ModelVariantCatalog, TranscribeError, WhisperModelVariant};

use super::ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
};

/// Configuration for whisper model acquisition (legacy FP16 single-model path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOrchestratorConfig {
    pub model_url: String,
    pub expected_sha256: String,
}

impl ModelOrchestratorConfig {
    /// Builds FP16 config from the canonical catalog.
    pub fn fp16_from_catalog() -> Self {
        let descriptor = ModelVariantCatalog::fp16();
        Self {
            model_url: descriptor.url.to_string(),
            expected_sha256: descriptor.expected_sha256.to_string(),
        }
    }
}

/// Orchestrates local model verification and download when required.
pub struct ModelOrchestrator<S, D> {
    store: S,
    downloader: D,
    selected_variant: WhisperModelVariant,
    active_variant: Option<WhisperModelVariant>,
    pending_variant: Option<WhisperModelVariant>,
}

impl<S, D> ModelOrchestrator<S, D> {
    pub fn new(store: S, downloader: D) -> Self {
        Self {
            store,
            downloader,
            selected_variant: WhisperModelVariant::default(),
            active_variant: None,
            pending_variant: None,
        }
    }

    pub fn selected_variant(&self) -> WhisperModelVariant {
        self.selected_variant
    }

    pub fn active_variant(&self) -> Option<WhisperModelVariant> {
        self.active_variant
    }

    pub fn pending_variant(&self) -> Option<WhisperModelVariant> {
        self.pending_variant
    }

    pub fn set_pending_variant(&mut self, variant: WhisperModelVariant) {
        self.pending_variant = Some(variant);
    }

    pub fn clear_pending_variant(&mut self) {
        self.pending_variant = None;
    }

    /// Sets the startup/restored variant before the first `ensure_model` call.
    pub fn initialize_selected_variant(&mut self, variant: WhisperModelVariant) {
        self.selected_variant = variant;
    }
}

impl<S: ModelStorePort, D> ModelOrchestrator<S, D> {
    /// Returns whether each catalog variant's file exists locally (existence only, not verified).
    pub fn local_availability(&self) -> std::collections::HashMap<WhisperModelVariant, bool> {
        ModelVariantCatalog::all()
            .iter()
            .map(|descriptor| {
                (
                    descriptor.variant,
                    self.store.file_exists(descriptor.variant),
                )
            })
            .collect()
    }
}

impl<S: ModelStorePort, D: ModelDownloaderPort> ModelOrchestrator<S, D> {
    /// Ensures the currently selected variant exists locally (legacy entry point).
    pub fn ensure_model<F>(&self, on_progress: F) -> Result<PathBuf, TranscribeError>
    where
        F: FnMut(ModelDownloadProgress),
    {
        self.ensure_variant(self.selected_variant, on_progress)
    }

    /// Ensures a specific variant exists locally, downloading when missing or corrupt.
    pub fn ensure_variant<F>(
        &self,
        variant: WhisperModelVariant,
        mut on_progress: F,
    ) -> Result<PathBuf, TranscribeError>
    where
        F: FnMut(ModelDownloadProgress),
    {
        let descriptor = ModelVariantCatalog::get(variant);
        if let Ok(path) = self
            .store
            .verify_variant(variant, Some(descriptor.expected_sha256))
        {
            return Ok(path);
        }

        if descriptor.url.is_empty() {
            return Err(TranscribeError::ModelNotFound {
                detail: format!(
                    "model file not found for {:?} and download URL is not configured",
                    variant
                ),
            });
        }

        let destination = self.store.model_path_for(variant);
        self.downloader
            .download(descriptor.url, &destination, &mut on_progress)?;

        emit_progress(&mut on_progress, ModelDownloadStatus::Verifying);
        self.store
            .verify_variant(variant, Some(descriptor.expected_sha256))
    }

    /// Selects a variant and ensures it is available.
    ///
    /// When `defer_to_cycle_boundary` is true (transcribing), the variant is staged as
    /// `pending_variant` and applied on the next batch cycle via [`try_apply_pending_variant`].
    /// Same variant as active is a verify-only no-op (no download/reload).
    pub fn apply_variant<F>(
        &mut self,
        variant: WhisperModelVariant,
        defer_to_cycle_boundary: bool,
        on_progress: F,
    ) -> Result<ApplyVariantOutcome, TranscribeError>
    where
        F: FnMut(ModelDownloadProgress),
    {
        self.selected_variant = variant;
        if self.active_variant == Some(variant) {
            let descriptor = ModelVariantCatalog::get(variant);
            let path = self
                .store
                .verify_variant(variant, Some(descriptor.expected_sha256))?;
            return Ok(ApplyVariantOutcome::NoOp { path });
        }
        let path = self.ensure_variant(variant, on_progress)?;
        if defer_to_cycle_boundary {
            self.pending_variant = Some(variant);
            return Ok(ApplyVariantOutcome::Deferred { path });
        }
        self.active_variant = Some(variant);
        self.pending_variant = None;
        Ok(ApplyVariantOutcome::Applied { path })
    }

    /// Applies a staged `pending_variant` at a batch cycle boundary.
    ///
    /// Returns the verified model path when a pending switch was committed.
    pub fn try_apply_pending_variant(
        &mut self,
    ) -> Result<Option<ApplyVariantOutcome>, TranscribeError> {
        let pending = match self.pending_variant {
            Some(variant) => variant,
            None => return Ok(None),
        };
        if self.active_variant == Some(pending) {
            self.pending_variant = None;
            return Ok(None);
        }
        let descriptor = ModelVariantCatalog::get(pending);
        let path = self
            .store
            .verify_variant(pending, Some(descriptor.expected_sha256))?;
        self.active_variant = Some(pending);
        self.pending_variant = None;
        Ok(Some(ApplyVariantOutcome::Applied { path }))
    }

    /// Marks the active variant after an external load path (e.g. transcribe orchestrator).
    pub fn mark_active_variant(&mut self, variant: WhisperModelVariant) {
        self.active_variant = Some(variant);
        self.selected_variant = variant;
        self.pending_variant = None;
    }
}

/// Result of a variant apply attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyVariantOutcome {
    /// Variant unchanged; verified local path only.
    NoOp { path: PathBuf },
    /// Variant ensured but deferred until the next batch cycle.
    Deferred { path: PathBuf },
    /// Variant applied immediately (path ready for worker reload).
    Applied { path: PathBuf },
}

impl ApplyVariantOutcome {
    pub fn path(&self) -> &Path {
        match self {
            Self::NoOp { path } | Self::Deferred { path } | Self::Applied { path } => path,
        }
    }
}

fn emit_progress<F>(on_progress: &mut F, status: ModelDownloadStatus)
where
    F: FnMut(ModelDownloadProgress),
{
    on_progress(ModelDownloadProgress {
        bytes_downloaded: 0,
        bytes_total: None,
        percent: None,
        status,
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use gijirec_domain::transcribe::{TranscribeErrorCode, WhisperModelVariant};

    use super::*;
    use crate::transcribe::test_support::{
        QueueModelDownloader, QueueModelStore, default_model_path,
    };

    fn orchestrator(
        store: Arc<QueueModelStore>,
        downloader: Arc<QueueModelDownloader>,
    ) -> ModelOrchestrator<Arc<QueueModelStore>, Arc<QueueModelDownloader>> {
        ModelOrchestrator::new(store, downloader)
    }

    fn ensure_model(
        store: Arc<QueueModelStore>,
        downloader: Arc<QueueModelDownloader>,
    ) -> Result<PathBuf, TranscribeError> {
        orchestrator(store, downloader).ensure_model(|_| {})
    }

    fn queue_store(
        model_path: PathBuf,
        verify_results: Vec<Result<PathBuf, TranscribeError>>,
    ) -> Arc<QueueModelStore> {
        QueueModelStore::new(model_path, verify_results)
    }

    fn assert_download_and_verify_counts(
        store: &Arc<QueueModelStore>,
        downloader: &Arc<QueueModelDownloader>,
        downloads: usize,
        verifications: usize,
    ) {
        assert_eq!(downloader.download_call_count(), downloads);
        assert_eq!(store.verify_call_count(), verifications);
    }

    fn model_not_found(detail: &str) -> TranscribeError {
        TranscribeError::ModelNotFound {
            detail: detail.to_string(),
        }
    }

    fn store_missing_then(
        second_verify: TranscribeError,
    ) -> (Arc<QueueModelStore>, Arc<QueueModelDownloader>) {
        let store = queue_store(
            default_model_path(),
            vec![Err(model_not_found("missing")), Err(second_verify)],
        );
        let downloader = QueueModelDownloader::success();
        (store, downloader)
    }

    #[test]
    fn existing_valid_model_skips_download() {
        let model_path = default_model_path();
        let store = queue_store(model_path.clone(), vec![Ok(model_path.clone())]);
        let downloader = Arc::new(QueueModelDownloader::success());

        let path = ensure_model(Arc::clone(&store), Arc::clone(&downloader))
            .expect("valid model should succeed");

        assert_eq!(path, model_path);
        assert_eq!(store.verify_call_count(), 1);
        assert_eq!(downloader.download_call_count(), 0);
    }

    #[test]
    fn missing_model_triggers_download_and_progress_callback() {
        let model_path = default_model_path();
        let store = QueueModelStore::new(
            model_path.clone(),
            vec![
                Err(TranscribeError::ModelNotFound {
                    detail: "missing".to_string(),
                }),
                Ok(model_path.clone()),
            ],
        );
        let downloader = QueueModelDownloader::success();
        let progress = Arc::new(Mutex::new(Vec::new()));

        let path = orchestrator(Arc::clone(&store), Arc::clone(&downloader))
            .ensure_model(|update| progress.lock().expect("lock").push(update))
            .expect("download should succeed");

        assert_eq!(path, model_path);
        assert_eq!(store.verify_call_count(), 2);
        assert_eq!(downloader.download_call_count(), 1);

        let updates = progress.lock().expect("lock");
        assert!(
            updates
                .iter()
                .any(|p| p.status == ModelDownloadStatus::Downloading),
            "must emit downloading progress"
        );
        assert!(
            updates
                .iter()
                .any(|p| p.status == ModelDownloadStatus::Verifying),
            "must emit verifying progress after download"
        );
    }

    #[test]
    fn corrupted_model_triggers_redownload_and_verification() {
        let model_path = default_model_path();
        let store = queue_store(
            model_path.clone(),
            vec![
                Err(TranscribeError::ModelCorrupt {
                    detail: "checksum mismatch".to_string(),
                }),
                Ok(model_path.clone()),
            ],
        );
        let downloader = Arc::new(QueueModelDownloader::success());

        let path = ensure_model(Arc::clone(&store), Arc::clone(&downloader))
            .expect("redownload should succeed");

        assert_eq!(path, model_path);
        assert_eq!(store.verify_call_count(), 2);
        assert_eq!(downloader.download_call_count(), 1);
    }

    #[test]
    fn download_failure_returns_model_download_failed() {
        let model_path = default_model_path();
        let store = queue_store(
            model_path,
            vec![Err(TranscribeError::ModelNotFound {
                detail: "missing".to_string(),
            })],
        );
        let downloader = Arc::new(QueueModelDownloader::failure(
            TranscribeError::ModelDownloadFailed {
                detail: "network timeout".to_string(),
            },
        ));

        let err = ensure_model(Arc::clone(&store), Arc::clone(&downloader))
            .expect_err("download failure should propagate");

        assert!(matches!(err, TranscribeError::ModelDownloadFailed { .. }));
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::ModelDownloadFailed
        );
        assert_download_and_verify_counts(&store, &downloader, 1, 1);
    }

    #[test]
    fn post_download_checksum_failure_returns_model_corrupt() {
        let (store, downloader) = store_missing_then(TranscribeError::ModelCorrupt {
            detail: "checksum mismatch after download".to_string(),
        });

        let err = ensure_model(Arc::clone(&store), Arc::clone(&downloader))
            .expect_err("post-download checksum failure should propagate");

        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        assert_eq!(err.to_user_facing().code, TranscribeErrorCode::ModelCorrupt);
        assert_download_and_verify_counts(&store, &downloader, 1, 2);
    }

    #[test]
    fn offline_first_launch_without_local_model_attempts_catalog_download() {
        let (store, downloader) =
            store_missing_then(model_not_found("still missing after download"));

        let err = ensure_model(Arc::clone(&store), Arc::clone(&downloader))
            .expect_err("missing local model should attempt download then verify");

        assert!(matches!(err, TranscribeError::ModelNotFound { .. }));
        assert_download_and_verify_counts(&store, &downloader, 1, 2);
    }

    #[test]
    fn apply_same_active_variant_skips_download() {
        let model_path = default_model_path();
        let store = QueueModelStore::new(model_path.clone(), vec![Ok(model_path.clone())]);
        let downloader = QueueModelDownloader::success();
        let mut orchestrator = orchestrator(Arc::clone(&store), Arc::clone(&downloader));
        orchestrator.mark_active_variant(WhisperModelVariant::Fp16);

        let path = orchestrator
            .apply_variant(WhisperModelVariant::Fp16, false, |_| {})
            .expect("same variant should verify only");

        assert_eq!(path, ApplyVariantOutcome::NoOp { path: model_path });
        assert_eq!(store.verify_call_count(), 1);
        assert_eq!(downloader.download_call_count(), 0);
    }

    #[test]
    fn apply_variant_deferred_when_cycle_boundary_requested() {
        struct VariantPathsStore {
            fp16: PathBuf,
            q8: PathBuf,
        }

        impl ModelStorePort for VariantPathsStore {
            fn model_path(&self) -> PathBuf {
                self.fp16.clone()
            }

            fn model_path_for(&self, variant: WhisperModelVariant) -> PathBuf {
                match variant {
                    WhisperModelVariant::Fp16 => self.fp16.clone(),
                    WhisperModelVariant::Q8_0 => self.q8.clone(),
                    WhisperModelVariant::Q5_0 => self.q8.clone(),
                }
            }

            fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
                self.verify_variant(WhisperModelVariant::Fp16, expected_sha256)
            }

            fn verify_variant(
                &self,
                variant: WhisperModelVariant,
                _expected_sha256: Option<&str>,
            ) -> Result<PathBuf, TranscribeError> {
                Ok(self.model_path_for(variant))
            }

            fn file_exists(&self, _variant: WhisperModelVariant) -> bool {
                true
            }
        }

        let fp16_path = PathBuf::from("/tmp/models/fp16.bin");
        let q8_path = PathBuf::from("/tmp/models/q8.bin");
        let store = VariantPathsStore {
            fp16: fp16_path.clone(),
            q8: q8_path.clone(),
        };
        let downloader = QueueModelDownloader::success();
        let mut orchestrator = ModelOrchestrator::new(store, downloader);
        orchestrator.mark_active_variant(WhisperModelVariant::Fp16);

        let outcome = orchestrator
            .apply_variant(WhisperModelVariant::Q8_0, true, |_| {})
            .expect("deferred apply");

        assert_eq!(
            outcome,
            ApplyVariantOutcome::Deferred {
                path: q8_path.clone(),
            }
        );
        assert_eq!(
            orchestrator.active_variant(),
            Some(WhisperModelVariant::Fp16)
        );
        assert_eq!(
            orchestrator.pending_variant(),
            Some(WhisperModelVariant::Q8_0)
        );

        let applied = orchestrator
            .try_apply_pending_variant()
            .expect("apply pending")
            .expect("pending outcome");
        assert_eq!(
            applied,
            ApplyVariantOutcome::Applied {
                path: q8_path.clone(),
            }
        );
        assert_eq!(
            orchestrator.active_variant(),
            Some(WhisperModelVariant::Q8_0)
        );
        assert_eq!(orchestrator.pending_variant(), None);
    }

    #[test]
    fn fp16_catalog_config_matches_contract_sha() {
        let config = ModelOrchestratorConfig::fp16_from_catalog();
        assert_eq!(
            config.expected_sha256,
            ModelVariantCatalog::fp16().expected_sha256
        );
        assert_eq!(config.model_url, ModelVariantCatalog::fp16().url);
    }
}
