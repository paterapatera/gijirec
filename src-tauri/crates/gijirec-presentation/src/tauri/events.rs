//! Tauri event emission for capture phase and user-facing errors.

use gijirec_domain::audio::{CaptureError, CapturePhase, UserFacingError};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Runtime};

/// Tauri event name for phase transitions.
pub const PHASE_CHANGED_EVENT: &str = "audio-capture://phase-changed";

/// Tauri event name for user-facing capture errors.
pub const ERROR_EVENT: &str = "audio-capture://error";

/// Payload per `docs/contracts/audio-capture-status.md`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CapturePhaseChangedPayload {
    pub phase: String,
    pub timestamp_ms: u64,
}

/// Payload per `docs/contracts/audio-capture-status.md`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CaptureUserErrorPayload {
    pub code: String,
    pub message_ja: String,
    pub action_ja: String,
    pub recoverable: bool,
}

/// Emits capture lifecycle events to the Tauri frontend.
pub trait CaptureEventEmitter: Send + Sync {
    fn emit_phase_changed(&self, phase: CapturePhase) -> Result<(), EmitError>;
    fn emit_error(&self, error: CaptureError) -> Result<(), EmitError>;
}

/// Errors when emitting Tauri events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    EmitFailed(String),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmitFailed(message) => write!(f, "failed to emit tauri event: {message}"),
        }
    }
}

impl std::error::Error for EmitError {}

/// Production emitter backed by [`AppHandle`].
pub struct TauriCaptureEventEmitter<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriCaptureEventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> CaptureEventEmitter for TauriCaptureEventEmitter<R> {
    fn emit_phase_changed(&self, phase: CapturePhase) -> Result<(), EmitError> {
        let payload = build_phase_payload(phase);
        self.app
            .emit(PHASE_CHANGED_EVENT, payload)
            .map_err(|err| EmitError::EmitFailed(err.to_string()))
    }

    fn emit_error(&self, error: CaptureError) -> Result<(), EmitError> {
        let payload = build_error_payload(error);
        self.app
            .emit(ERROR_EVENT, payload)
            .map_err(|err| EmitError::EmitFailed(err.to_string()))
    }
}

pub fn build_phase_payload(phase: CapturePhase) -> CapturePhaseChangedPayload {
    CapturePhaseChangedPayload {
        phase: phase.as_str().to_string(),
        timestamp_ms: current_timestamp_ms(),
    }
}

pub fn build_error_payload(error: CaptureError) -> CaptureUserErrorPayload {
    let facing: UserFacingError = error.to_user_facing();
    CaptureUserErrorPayload {
        code: facing.code.as_str().to_string(),
        message_ja: facing.message_ja,
        action_ja: facing.action_ja,
        recoverable: facing.recoverable,
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// In-memory recorder for unit tests.
#[derive(Debug, Default)]
pub struct RecordingEventEmitter {
    phases: Arc<Mutex<Vec<CapturePhaseChangedPayload>>>,
    errors: Arc<Mutex<Vec<CaptureUserErrorPayload>>>,
}

impl RecordingEventEmitter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn phases(&self) -> Vec<CapturePhaseChangedPayload> {
        self.phases.lock().expect("lock").clone()
    }

    pub fn errors(&self) -> Vec<CaptureUserErrorPayload> {
        self.errors.lock().expect("lock").clone()
    }
}

impl CaptureEventEmitter for RecordingEventEmitter {
    fn emit_phase_changed(&self, phase: CapturePhase) -> Result<(), EmitError> {
        self.phases
            .lock()
            .expect("lock")
            .push(build_phase_payload(phase));
        Ok(())
    }

    fn emit_error(&self, error: CaptureError) -> Result<(), EmitError> {
        self.errors
            .lock()
            .expect("lock")
            .push(build_error_payload(error));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::UserFacingErrorCode;

    #[test]
    fn phase_payload_uses_contract_strings() {
        let payload = build_phase_payload(CapturePhase::Capturing);
        assert_eq!(payload.phase, "capturing");
    }

    #[test]
    fn error_payload_includes_action_ja() {
        let payload = build_error_payload(CaptureError::MicPermissionDenied);
        assert_eq!(
            payload.code,
            UserFacingErrorCode::MicPermissionDenied.as_str()
        );
        assert!(!payload.action_ja.is_empty());
        assert!(!payload.message_ja.is_empty());
        assert!(payload.recoverable);
    }

    #[test]
    fn transcribe_ingest_no_audio_source_error_payload_matches_contract() {
        let payload = build_error_payload(CaptureError::TranscribeIngestNoAudioSource);
        assert_eq!(payload.code, "TRANSCRIBE_INGEST_NO_AUDIO_SOURCE");
        assert_eq!(payload.message_ja, "転写に利用できる音声源がありません。");
        assert_eq!(
            payload.action_ja,
            "スピーカー出力を確認するか、マイク ingest を ON にしてください"
        );
        assert!(payload.recoverable);
    }

    #[test]
    fn recording_emitter_captures_phase_and_error() {
        let emitter = RecordingEventEmitter::new();
        emitter
            .emit_phase_changed(CapturePhase::Starting)
            .expect("phase");
        emitter
            .emit_error(CaptureError::SystemAudioUnavailable)
            .expect("error");

        assert_eq!(emitter.phases().len(), 1);
        assert_eq!(emitter.phases()[0].phase, "starting");
        assert_eq!(emitter.errors().len(), 1);
        assert_eq!(emitter.errors()[0].code, "SYSTEM_AUDIO_UNAVAILABLE");
        assert!(!emitter.errors()[0].action_ja.is_empty());
    }
}
