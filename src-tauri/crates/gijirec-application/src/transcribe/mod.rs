//! Whisper transcribe orchestration ports and services.
pub mod block_emitter;
pub mod model_orchestrator;
pub mod orchestrator;
pub mod ports;
pub mod settings_service;

pub use block_emitter::BlockEmitter;
pub use model_orchestrator::{ApplyVariantOutcome, ModelOrchestrator, ModelOrchestratorConfig};
pub use orchestrator::{DefaultTranscribeOrchestrator, TranscribeOrchestrator};
pub use ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
    TranscribeWorkerPort, WhisperContextPort,
};
pub use settings_service::TranscribeSettingsService;
