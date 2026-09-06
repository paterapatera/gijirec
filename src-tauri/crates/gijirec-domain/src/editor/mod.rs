//! Domain types for the transcript editor (save paths, document model).

pub mod error;
pub mod save;
pub mod settings;

pub use error::{EditorError, EditorUserError, EditorUserErrorCode};
pub use save::{
    AiTranscriptionJsonlRecord, SaveFileFailure, SaveTranscriptSessionRequest,
    SaveTranscriptSessionResult,
};
pub use settings::EditorSettings;

/// Compile-time marker that the editor module is linked into the domain crate.
pub const MODULE_STUB: &str = "editor";
