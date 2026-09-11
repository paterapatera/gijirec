//! Domain crate. Must not depend on application, infrastructure, or presentation.
#[cfg(test)]
mod consumer_contract;

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

#[cfg(any(test, feature = "contract-test-support"))]
#[allow(dead_code)]
pub mod user_facing_contract_tests;
