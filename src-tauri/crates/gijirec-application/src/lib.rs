//! Application crate. Depends on domain only.
pub use gijirec_domain as domain;

pub mod capture;
pub mod capture_audio_controls;
pub mod device_selection;
pub mod editor;
pub(crate) mod settings_file;
pub mod transcribe;
pub(crate) mod user_facing_error;

pub use transcribe::{
    ApplyVariantOutcome, BlockEmitter, DefaultTranscribeOrchestrator, ModelOrchestrator,
    ModelOrchestratorConfig, TranscribeOrchestrator, TranscribeSettingsService,
};
