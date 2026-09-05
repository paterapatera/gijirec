//! Application crate. Depends on domain only.
pub use gijirec_domain as domain;

pub mod capture;
pub mod transcribe;

pub use transcribe::{
    BlockEmitter, DefaultTranscribeOrchestrator, ModelOrchestrator, ModelOrchestratorConfig,
    TranscribeOrchestrator,
};
