//! Tauri event emission for transcribe phase, model progress, and user errors.

use gijirec_application::transcribe::{ModelDownloadProgress, ModelDownloadStatus};
use gijirec_domain::transcribe::{TranscribeError, TranscribePhase, UserFacingTranscribeError};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

/// Tauri event name for transcribe phase transitions.
pub const TRANSCRIBE_PHASE_CHANGED_EVENT: &str = "whisper-transcribe://phase-changed";

/// Tauri event name for model download/verification progress.
pub const TRANSCRIBE_MODEL_PROGRESS_EVENT: &str = "whisper-transcribe://model-progress";

/// Tauri event name for user-facing transcribe errors.
pub const TRANSCRIBE_ERROR_EVENT: &str = "whisper-transcribe://error";

/// Clock source for capture/session-relative timestamp.
pub type TimestampClock = std::sync::Arc<dyn Fn() -> u64 + Send + Sync>;

/// Payload for `whisper-transcribe://phase-changed` per contract.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TranscribePhaseChangedPayload {
    pub phase: String,
    pub timestamp_ms: u64,
}

/// Payload for `whisper-transcribe://model-progress` per contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TranscribeModelProgressPayload {
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub percent: Option<f64>,
    pub status: String,
}

impl From<&ModelDownloadProgress> for TranscribeModelProgressPayload {
    fn from(p: &ModelDownloadProgress) -> Self {
        let status = match p.status {
            ModelDownloadStatus::Downloading => "downloading",
            ModelDownloadStatus::Verifying => "verifying",
            ModelDownloadStatus::Complete => "complete",
            ModelDownloadStatus::Failed => "failed",
        };
        Self {
            bytes_downloaded: p.bytes_downloaded,
            bytes_total: p.bytes_total,
            percent: p.percent,
            status: status.to_string(),
        }
    }
}

/// Errors when emitting transcribe events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeEmitError {
    EmitFailed(String),
}

impl std::fmt::Display for TranscribeEmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmitFailed(msg) => write!(f, "failed to emit transcribe event: {msg}"),
        }
    }
}

impl std::error::Error for TranscribeEmitError {}

/// Abstract emitter interface for UI events.
pub trait TranscribeEventEmitter: Send + Sync {
    fn emit_phase_changed(&self, phase: TranscribePhase) -> Result<(), TranscribeEmitError>;
    fn emit_model_progress(
        &self,
        progress: &ModelDownloadProgress,
    ) -> Result<(), TranscribeEmitError>;
    fn emit_error(&self, error: &TranscribeError) -> Result<(), TranscribeEmitError>;
}

/// Production implementation backed by [`AppHandle`].
pub struct TauriTranscribeEventEmitter<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
    clock: Option<TimestampClock>,
}

impl<R: Runtime> TauriTranscribeEventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app, clock: None }
    }

    pub fn with_clock(app: AppHandle<R>, clock: TimestampClock) -> Self {
        Self {
            app,
            clock: Some(clock),
        }
    }

    pub fn set_clock(&mut self, clock: TimestampClock) {
        self.clock = Some(clock);
    }

    fn now_timestamp_ms(&self) -> u64 {
        self.clock.as_ref().map(|c| c()).unwrap_or(0)
    }
}

impl<R: Runtime> TranscribeEventEmitter for TauriTranscribeEventEmitter<R> {
    fn emit_phase_changed(&self, phase: TranscribePhase) -> Result<(), TranscribeEmitError> {
        let payload = TranscribePhaseChangedPayload {
            phase: phase.as_str().to_string(),
            timestamp_ms: self.now_timestamp_ms(),
        };
        self.app
            .emit(TRANSCRIBE_PHASE_CHANGED_EVENT, payload)
            .map_err(|e| TranscribeEmitError::EmitFailed(e.to_string()))
    }

    fn emit_model_progress(
        &self,
        progress: &ModelDownloadProgress,
    ) -> Result<(), TranscribeEmitError> {
        let payload = TranscribeModelProgressPayload::from(progress);
        self.app
            .emit(TRANSCRIBE_MODEL_PROGRESS_EVENT, payload)
            .map_err(|e| TranscribeEmitError::EmitFailed(e.to_string()))
    }

    fn emit_error(&self, error: &TranscribeError) -> Result<(), TranscribeEmitError> {
        let user_facing: UserFacingTranscribeError = error.to_user_facing();
        self.app
            .emit(TRANSCRIBE_ERROR_EVENT, user_facing)
            .map_err(|e| TranscribeEmitError::EmitFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::transcribe::TranscribeErrorCode;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingTranscribeEmitter {
        phases: Mutex<Vec<TranscribePhaseChangedPayload>>,
        model_progress: Mutex<Vec<TranscribeModelProgressPayload>>,
        errors: Mutex<Vec<UserFacingTranscribeError>>,
        clock_time: Mutex<u64>,
    }

    impl RecordingTranscribeEmitter {
        fn new() -> Self {
            Self::default()
        }

        fn set_time(&self, time_ms: u64) {
            *self.clock_time.lock().unwrap() = time_ms;
        }
    }

    impl TranscribeEventEmitter for RecordingTranscribeEmitter {
        fn emit_phase_changed(&self, phase: TranscribePhase) -> Result<(), TranscribeEmitError> {
            let timestamp_ms = *self.clock_time.lock().unwrap();
            self.phases
                .lock()
                .unwrap()
                .push(TranscribePhaseChangedPayload {
                    phase: phase.as_str().to_string(),
                    timestamp_ms,
                });
            Ok(())
        }

        fn emit_model_progress(
            &self,
            progress: &ModelDownloadProgress,
        ) -> Result<(), TranscribeEmitError> {
            self.model_progress
                .lock()
                .unwrap()
                .push(TranscribeModelProgressPayload::from(progress));
            Ok(())
        }

        fn emit_error(&self, error: &TranscribeError) -> Result<(), TranscribeEmitError> {
            self.errors.lock().unwrap().push(error.to_user_facing());
            Ok(())
        }
    }

    #[test]
    fn emits_phase_changed_with_contract_string_and_timestamp() {
        let emitter = RecordingTranscribeEmitter::new();
        emitter.set_time(2500);

        emitter
            .emit_phase_changed(TranscribePhase::LoadingModel)
            .unwrap();
        emitter.emit_phase_changed(TranscribePhase::Ready).unwrap();
        emitter
            .emit_phase_changed(TranscribePhase::Transcribing)
            .unwrap();

        let phases = emitter.phases.lock().unwrap().clone();
        assert_eq!(phases.len(), 3);
        assert_eq!(phases[0].phase, "loading_model");
        assert_eq!(phases[0].timestamp_ms, 2500);
        assert_eq!(phases[1].phase, "ready");
        assert_eq!(phases[2].phase, "transcribing");
    }

    #[test]
    fn emits_model_progress_with_nullables_and_string_status() {
        let emitter = RecordingTranscribeEmitter::new();

        let progress = ModelDownloadProgress {
            bytes_downloaded: 1024 * 1024,
            bytes_total: Some(10 * 1024 * 1024),
            percent: Some(10.0),
            status: ModelDownloadStatus::Downloading,
        };
        emitter.emit_model_progress(&progress).unwrap();

        let records = emitter.model_progress.lock().unwrap().clone();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].bytes_downloaded, 1024 * 1024);
        assert_eq!(records[0].bytes_total, Some(10 * 1024 * 1024));
        assert_eq!(records[0].percent, Some(10.0));
        assert_eq!(records[0].status, "downloading");

        // Verify status mapping
        let verify_progress = ModelDownloadProgress {
            bytes_downloaded: 10 * 1024 * 1024,
            bytes_total: Some(10 * 1024 * 1024),
            percent: Some(100.0),
            status: ModelDownloadStatus::Verifying,
        };
        emitter.emit_model_progress(&verify_progress).unwrap();
        let records = emitter.model_progress.lock().unwrap().clone();
        assert_eq!(records[1].status, "verifying");
    }

    #[test]
    fn emits_error_with_action_ja_and_no_raw_data() {
        let emitter = RecordingTranscribeEmitter::new();

        let err = TranscribeError::ModelCorrupt {
            detail: "invalid magic byte in header".to_string(),
        };
        emitter.emit_error(&err).unwrap();

        let errors = emitter.errors.lock().unwrap().clone();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, TranscribeErrorCode::ModelCorrupt);
        assert!(errors[0].action_ja_is_present());
        assert!(!errors[0].message_ja.is_empty());
        assert!(!errors[0].action_ja.contains("magic byte")); // Detail not exposed to user
    }
}
