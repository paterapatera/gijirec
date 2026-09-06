//! Domain crate. Must not depend on application, infrastructure, or presentation.
pub mod audio;
pub mod editor;
pub mod transcribe;

pub use transcribe::{
    PhaseTransitionError, TranscribeError, TranscribeErrorCode, TranscribePhase,
    UserFacingTranscribeError,
};

#[cfg(test)]
mod editor_compile_test;
