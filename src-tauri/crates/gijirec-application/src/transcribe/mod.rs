//! Whisper transcribe orchestration ports and services.
pub mod block_emitter;
pub mod model_orchestrator;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod noop_ports;
pub mod orchestrator;
pub mod ports;
pub mod settings_service;

#[cfg(test)]
pub(crate) mod test_support;

pub use block_emitter::BlockEmitter;
pub use model_orchestrator::{ApplyVariantOutcome, ModelOrchestrator, ModelOrchestratorConfig};
pub use orchestrator::{DefaultTranscribeOrchestrator, TranscribeOrchestrator};
pub use ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
    TranscribeWorkerPort, WhisperContextPort,
};
pub use settings_service::TranscribeSettingsService;
