//! Whisper transcribe presentation adapters.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub mod event_emitter;
pub mod ingest_level_emitter;
pub mod lifecycle_hook;
pub mod observability;
pub mod pcm_ingest_consumer;
pub mod settings_commands;
pub mod stall_watchdog;
pub mod status_cache;
pub mod transcript_block_bus;

pub use event_emitter::{
    TRANSCRIBE_ERROR_EVENT, TRANSCRIBE_MODEL_PROGRESS_EVENT, TRANSCRIBE_PHASE_CHANGED_EVENT,
    TauriTranscribeEventEmitter, TranscribeEmitError, TranscribeEventEmitter,
    TranscribeModelProgressPayload, TranscribePhaseChangedPayload,
};
pub use gijirec_application::transcribe::{
    ApplyVariantOutcome, ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort,
    ModelOrchestrator, ModelOrchestratorConfig, ModelStorePort, TranscribeSettingsService,
    TranscribeWorkerPort, WhisperContextPort,
};
pub use gijirec_domain::transcribe::{TranscribeError, TranscriptSegmentSink, WhisperModelVariant};
pub use gijirec_infrastructure::transcribe::{
    ModelDownloader, ModelStore, TranscribeWorker, WhisperCppAdapter,
};
pub use ingest_level_emitter::{
    AGGREGATION_WINDOW, DBFS_FLOOR, INGEST_LEVEL_EVENT, IngestLevelChangedPayload,
    IngestLevelEmitter, IngestLevelEventEmitter, MAX_EMIT_INTERVAL, MIN_EMIT_INTERVAL,
    TauriIngestLevelEventEmitter, aggregate_window_rms, rms_to_dbfs,
};
pub use lifecycle_hook::{
    DEFAULT_TRANSCRIBE_STOP_TIMEOUT, TRANSCRIBE_STOP_INFERENCE_MARGIN, TranscribeLifecycleHook,
};
pub use observability::{
    TRANSCRIBE_LOG_TARGET, TranscribeObservability, log_model_variant_applied,
    log_model_variant_selected, set_transcribe_observability,
};
pub use pcm_ingest_consumer::{PcmIngestConsumer, SequenceGapCallback};
pub use settings_commands::{
    GetTranscribeSettingsResponse, SetTranscribeModelVariantResponse,
    apply_transcribe_model_variant_impl, get_transcribe_settings_impl, invalid_model_variant_error,
    persist_transcribe_model_variant,
};
pub use stall_watchdog::{
    BATCH_INTERVAL, OrchestratorStallAdapter, SILENCE_RMS_THRESHOLD, STALL_POLL_INTERVAL,
    STALL_THRESHOLD, SharedTranscribeEmitter, StallClock, StallWatchdogRuntime,
    TranscribeStallOrchestrator, TranscribeStallWatchdog, chunk_rms,
};
pub use status_cache::{TranscribeStatusCache, TranscribeStatusSnapshot};
pub use transcript_block_bus::{
    BLOCK_APPENDED_EVENT, BlockDropCallback, MAX_QUEUED_BLOCKS, TauriTranscriptBlockEventEmitter,
    TimestampClock, TranscriptBlockAppendedPayload, TranscriptBlockBus,
    TranscriptBlockEventEmitter, TranscriptBlockPayload,
};

/// Presentation bridge implementing [`WhisperContextPort`] for [`WhisperCppAdapter`].
pub struct WhisperContextPortAdapter(WhisperCppAdapter);

impl WhisperContextPortAdapter {
    pub fn new() -> Self {
        Self(WhisperCppAdapter::new())
    }

    pub fn inner(&self) -> &WhisperCppAdapter {
        &self.0
    }

    pub fn inner_mut(&mut self) -> &mut WhisperCppAdapter {
        &mut self.0
    }
}

impl Default for WhisperContextPortAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WhisperContextPort for WhisperContextPortAdapter {
    fn load_model(&mut self, path: &Path) -> Result<(), TranscribeError> {
        self.0.load_model(path)
    }
}

/// Presentation bridge implementing [`ModelStorePort`] for [`ModelStore`].
pub struct ModelStorePortAdapter(ModelStore);

impl ModelStorePortAdapter {
    pub fn new(store: ModelStore) -> Self {
        Self(store)
    }

    pub fn inner(&self) -> &ModelStore {
        &self.0
    }
}

impl ModelStorePort for ModelStorePortAdapter {
    fn model_path(&self) -> PathBuf {
        self.0.model_path()
    }

    fn model_path_for(&self, variant: WhisperModelVariant) -> PathBuf {
        self.0.model_path_for(variant)
    }

    fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
        self.0.verify(expected_sha256)
    }

    fn verify_variant(
        &self,
        variant: WhisperModelVariant,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf, TranscribeError> {
        self.0.verify_variant(variant, expected_sha256)
    }

    fn file_exists(&self, variant: WhisperModelVariant) -> bool {
        self.0.file_exists(variant)
    }
}

/// Presentation bridge implementing [`ModelDownloaderPort`] for [`ModelDownloader`].
pub struct ModelDownloaderPortAdapter(ModelDownloader);

impl ModelDownloaderPortAdapter {
    pub fn new(downloader: ModelDownloader) -> Result<Self, TranscribeError> {
        Ok(Self(downloader))
    }

    pub fn inner(&self) -> &ModelDownloader {
        &self.0
    }
}

impl ModelDownloaderPort for ModelDownloaderPortAdapter {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        self.0.download(url, destination, |progress| {
            on_progress(map_download_progress(progress));
        })
    }
}

fn map_download_progress(
    progress: gijirec_infrastructure::transcribe::ModelDownloadProgress,
) -> ModelDownloadProgress {
    ModelDownloadProgress {
        bytes_downloaded: progress.bytes_downloaded,
        bytes_total: progress.bytes_total,
        percent: progress.percent,
        status: map_download_status(progress.status),
    }
}

fn map_download_status(
    status: gijirec_infrastructure::transcribe::ModelDownloadStatus,
) -> ModelDownloadStatus {
    match status {
        gijirec_infrastructure::transcribe::ModelDownloadStatus::Downloading => {
            ModelDownloadStatus::Downloading
        }
        gijirec_infrastructure::transcribe::ModelDownloadStatus::Verifying => {
            ModelDownloadStatus::Verifying
        }
        gijirec_infrastructure::transcribe::ModelDownloadStatus::Complete => {
            ModelDownloadStatus::Complete
        }
        gijirec_infrastructure::transcribe::ModelDownloadStatus::Failed => {
            ModelDownloadStatus::Failed
        }
    }
}

/// Presentation bridge implementing [`TranscribeWorkerPort`] for [`TranscribeWorker`].
pub struct TranscribeWorkerPortAdapter {
    inner: TranscribeWorker,
}

impl TranscribeWorkerPortAdapter {
    pub fn new(sink: Arc<dyn TranscriptSegmentSink>) -> Self {
        Self {
            inner: TranscribeWorker::new(sink),
        }
    }

    pub fn from_worker(worker: TranscribeWorker) -> Self {
        Self { inner: worker }
    }

    pub fn inner(&self) -> &TranscribeWorker {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut TranscribeWorker {
        &mut self.inner
    }

    pub fn attach_pcm_consumer(&mut self, consumer: rtrb::Consumer<f32>) {
        self.inner.attach_pcm_consumer(consumer);
    }

    pub fn install_whisper_adapter(&mut self, adapter: WhisperCppAdapter) {
        self.inner.install_engine(adapter);
    }
}

impl TranscribeWorkerPort for TranscribeWorkerPortAdapter {
    fn prepare_model_path(&mut self, path: &Path) -> Result<(), TranscribeError> {
        self.inner.prepare_model_path(path)
    }

    fn spawn(&mut self) -> Result<(), TranscribeError> {
        self.inner.spawn()
    }

    fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError> {
        self.inner.stop_and_join(timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_context_port_delegates_load_model() {
        let mut adapter: Box<dyn WhisperContextPort> = Box::new(WhisperContextPortAdapter::new());
        let err = adapter
            .load_model(Path::new("/nonexistent/gijirec-model.bin"))
            .expect_err("missing model should fail");
        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
    }

    #[test]
    fn whisper_context_port_is_object_safe() {
        let mut adapter: Box<dyn WhisperContextPort> = Box::new(WhisperContextPortAdapter::new());
        adapter
            .load_model(Path::new("/nonexistent/gijirec-model.bin"))
            .expect_err("load should fail");
    }

    #[test]
    fn model_store_port_delegates_verify_and_model_path() {
        let base = std::env::temp_dir().join(format!(
            "gijirec-model-store-adapter-{}",
            std::process::id()
        ));
        let store = ModelStore::new(base.clone());
        let adapter = ModelStorePortAdapter::new(store);

        let err = adapter.verify(None).expect_err("missing model should fail");
        assert!(matches!(err, TranscribeError::ModelNotFound { .. }));
        assert!(
            adapter
                .model_path()
                .ends_with("kotoba-whisper-v2.2-ggml.bin")
        );

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn model_downloader_port_is_object_safe() {
        let downloader =
            ModelDownloaderPortAdapter::new(ModelDownloader::new().expect("downloader"))
                .expect("adapter");
        let _: Box<dyn ModelDownloaderPort> = Box::new(downloader);
    }

    struct NoopSegmentSink;

    impl TranscriptSegmentSink for NoopSegmentSink {
        fn on_segment(
            &self,
            _text: &str,
            _start_ms: u64,
            _language: &str,
        ) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    #[test]
    fn transcribe_worker_port_prepare_model_path_does_not_load_until_spawn() {
        struct NoopSegmentSink;

        impl TranscriptSegmentSink for NoopSegmentSink {
            fn on_segment(
                &self,
                _text: &str,
                _start_ms: u64,
                _language: &str,
            ) -> Result<(), TranscribeError> {
                Ok(())
            }
        }

        let sink: Arc<dyn TranscriptSegmentSink> = Arc::new(NoopSegmentSink);
        let mut adapter = TranscribeWorkerPortAdapter::new(sink);
        assert!(!adapter.inner().is_engine_loaded());

        let err = adapter
            .prepare_model_path(Path::new("/nonexistent/gijirec-model.bin"))
            .expect_err("missing model should fail");
        assert!(matches!(err, TranscribeError::ModelNotFound { .. }));
        assert!(!adapter.inner().is_engine_loaded());
    }

    #[test]
    fn transcribe_worker_port_delegates_spawn_and_stop() {
        let sink: Arc<dyn TranscriptSegmentSink> = Arc::new(NoopSegmentSink);
        let mut adapter = TranscribeWorkerPortAdapter::new(sink);
        let (_prod, cons) = rtrb::RingBuffer::<f32>::new(1_024);
        adapter.attach_pcm_consumer(cons);

        adapter.spawn().expect("spawn");
        adapter.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn transcribe_worker_port_is_object_safe() {
        let sink: Arc<dyn TranscriptSegmentSink> = Arc::new(NoopSegmentSink);
        let adapter = TranscribeWorkerPortAdapter::new(sink);
        let _: Box<dyn TranscribeWorkerPort> = Box::new(adapter);
    }
}
