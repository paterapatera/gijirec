//! Application crate. Depends on domain only.
pub use gijirec_domain as domain;

pub mod capture;
pub mod capture_audio_controls;
pub mod device_selection;
pub mod editor;
pub mod transcribe;

pub use transcribe::{
    ApplyVariantOutcome, BlockEmitter, DefaultTranscribeOrchestrator, ModelOrchestrator,
    ModelOrchestratorConfig, TranscribeOrchestrator, TranscribeSettingsService,
};
