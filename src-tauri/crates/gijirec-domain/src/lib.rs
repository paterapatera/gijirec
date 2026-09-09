//! Domain crate. Must not depend on application, infrastructure, or presentation.
pub mod audio;
pub mod editor;
pub mod transcribe;

pub use transcribe::{
    ModelVariantCatalog, ModelVariantDescriptor, PhaseTransitionError, TranscribeError,
    TranscribeErrorCode, TranscribePhase, TranscribeSettings, TranscribeSettingsError,
    TranscribeSettingsErrorCode, TranscribeSettingsLoadIssue, TranscribeSettingsLoadResult,
    TranscribeSettingsUserError, UserFacingTranscribeError, WhisperModelVariant,
};

#[cfg(test)]
mod editor_compile_test;
