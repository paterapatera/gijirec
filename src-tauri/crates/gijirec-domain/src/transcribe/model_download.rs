//! Model download progress types per `docs/contracts/whisper-transcribe-status.md`.

use std::path::Path;

use super::error::TranscribeError;

/// Shared missing-model path for whisper adapter contract tests.
pub const MISSING_WHISPER_MODEL_PATH: &str = "/nonexistent/gijirec-model.bin";

/// `Path` view of [`MISSING_WHISPER_MODEL_PATH`] for load-model contract tests.
pub fn missing_whisper_model_path() -> &'static Path {
    Path::new(MISSING_WHISPER_MODEL_PATH)
}

/// Loads [`missing_whisper_model_path`] via `load` and asserts `ModelCorrupt`.
pub fn missing_whisper_model_load_err(
    load: impl FnOnce(&Path) -> Result<(), TranscribeError>,
) -> TranscribeError {
    let err = load(missing_whisper_model_path()).expect_err("missing model should fail");
    assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
    err
}

/// Progress payload emitted through download callbacks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModelDownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub percent: Option<f64>,
    pub status: ModelDownloadStatus,
}

/// Download lifecycle status emitted through progress callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelDownloadStatus {
    Downloading,
    Verifying,
    Complete,
    Failed,
}
