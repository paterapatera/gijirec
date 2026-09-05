//! Domain crate. Must not depend on application, infrastructure, or presentation.
pub mod audio;
pub mod transcribe;

pub use transcribe::{
    PhaseTransitionError, TranscribeError, TranscribeErrorCode, TranscribePhase,
    UserFacingTranscribeError,
};
