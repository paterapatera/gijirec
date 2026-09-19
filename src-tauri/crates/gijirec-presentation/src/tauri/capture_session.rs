//! Testable capture session command logic (`docs/contracts/capture-session-toggle.md`).
//!
//! Thin `#[tauri::command]` wrappers live in the host `commands` module (task 1.3).

use gijirec_application::capture_session::{
    CaptureSessionError, CaptureSessionServiceApi, CaptureSessionSnapshot,
};
use gijirec_domain::capture_session::CaptureSessionPhase;
use serde::{Deserialize, Serialize};

/// Tauri event name for session state changes.
pub const STATE_CHANGED_EVENT: &str = "capture-session://state-changed";

/// IPC response per `docs/contracts/capture-session-toggle.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSessionState {
    pub session_phase: CaptureSessionPhase,
    pub transition_busy: bool,
    pub capture_phase: String,
    pub timestamp_ms: u64,
}

/// Payload per `docs/contracts/capture-session-toggle.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSessionStateChangedPayload {
    pub state: CaptureSessionState,
}

/// Invoke error payload for `start_capture_session`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSessionInvokeError {
    pub code: String,
    pub message_ja: String,
    pub action_ja: String,
}

impl CaptureSessionInvokeError {
    pub fn from_service_error(err: CaptureSessionError) -> Self {
        Self {
            code: err.code.as_str().to_string(),
            message_ja: err.message_ja,
            action_ja: err.action_ja,
        }
    }
}

pub fn snapshot_to_ipc_state(snapshot: CaptureSessionSnapshot) -> CaptureSessionState {
    CaptureSessionState {
        session_phase: snapshot.session_phase,
        transition_busy: snapshot.transition_busy,
        capture_phase: snapshot.capture_phase.as_str().to_string(),
        timestamp_ms: snapshot.timestamp_ms,
    }
}

pub fn build_state_changed_payload(
    snapshot: CaptureSessionSnapshot,
) -> CaptureSessionStateChangedPayload {
    CaptureSessionStateChangedPayload {
        state: snapshot_to_ipc_state(snapshot),
    }
}

pub fn get_capture_session_state_impl(
    service: &dyn CaptureSessionServiceApi,
) -> CaptureSessionState {
    snapshot_to_ipc_state(service.get_state())
}

pub fn start_capture_session_impl(
    service: &dyn CaptureSessionServiceApi,
) -> Result<CaptureSessionState, CaptureSessionInvokeError> {
    service
        .start()
        .map(snapshot_to_ipc_state)
        .map_err(CaptureSessionInvokeError::from_service_error)
}

/// Production emitter backed by [`AppHandle`], implementing [`CaptureSessionEvents`].
pub mod emitter {
    use super::{
        CaptureSessionStateChangedPayload, STATE_CHANGED_EVENT, build_state_changed_payload,
    };
    use gijirec_application::capture_session::{CaptureSessionEvents, CaptureSessionSnapshot};
    use std::sync::{Arc, Mutex};
    use tauri::{AppHandle, Emitter, Runtime};

    pub struct TauriCaptureSessionEventEmitter<R: Runtime = tauri::Wry> {
        app: AppHandle<R>,
    }

    impl<R: Runtime> TauriCaptureSessionEventEmitter<R> {
        pub fn new(app: AppHandle<R>) -> Self {
            Self { app }
        }
    }

    impl<R: Runtime> CaptureSessionEvents for TauriCaptureSessionEventEmitter<R> {
        fn emit_state_changed(&self, state: &CaptureSessionSnapshot) {
            let payload = build_state_changed_payload(state.clone());
            let _ = self.app.emit(STATE_CHANGED_EVENT, payload);
        }
    }

    /// In-memory recorder for unit tests.
    #[derive(Debug, Default, Clone)]
    pub struct RecordingCaptureSessionEventEmitter {
        states: Arc<Mutex<Vec<CaptureSessionStateChangedPayload>>>,
    }

    impl RecordingCaptureSessionEventEmitter {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn state_changes(&self) -> Vec<CaptureSessionStateChangedPayload> {
            self.states.lock().expect("lock").clone()
        }
    }

    impl CaptureSessionEvents for RecordingCaptureSessionEventEmitter {
        fn emit_state_changed(&self, state: &CaptureSessionSnapshot) {
            self.states
                .lock()
                .expect("lock")
                .push(build_state_changed_payload(state.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emitter::RecordingCaptureSessionEventEmitter;
    use gijirec_application::capture_session::{CaptureSessionError, CaptureSessionEvents};
    use gijirec_domain::audio::CapturePhase;
    use gijirec_domain::capture_session::CaptureSessionErrorCode;
    use gijirec_domain::user_facing_contract_tests::assert_invoke_error_serializes_contract_shape;
    use std::sync::Mutex;

    struct MockCaptureSessionService {
        state: Mutex<CaptureSessionSnapshot>,
        start_error: Option<CaptureSessionError>,
    }

    impl MockCaptureSessionService {
        fn with_state(state: CaptureSessionSnapshot) -> Self {
            Self {
                state: Mutex::new(state),
                start_error: None,
            }
        }

        fn with_start_error(state: CaptureSessionSnapshot, err: CaptureSessionError) -> Self {
            Self {
                state: Mutex::new(state),
                start_error: Some(err),
            }
        }
    }

    impl CaptureSessionServiceApi for MockCaptureSessionService {
        fn get_state(&self) -> CaptureSessionSnapshot {
            self.state.lock().expect("lock").clone()
        }

        fn start(&self) -> Result<CaptureSessionSnapshot, CaptureSessionError> {
            if let Some(err) = &self.start_error {
                return Err(err.clone());
            }
            let mut state = self.state.lock().expect("lock");
            state.session_phase = CaptureSessionPhase::Active;
            Ok(state.clone())
        }
    }

    fn sample_snapshot() -> CaptureSessionSnapshot {
        CaptureSessionSnapshot {
            session_phase: CaptureSessionPhase::Idle,
            transition_busy: false,
            capture_phase: CapturePhase::Idle,
            timestamp_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn snapshot_to_ipc_state_maps_capture_phase_string() {
        let ipc = snapshot_to_ipc_state(CaptureSessionSnapshot {
            session_phase: CaptureSessionPhase::Active,
            transition_busy: false,
            capture_phase: CapturePhase::Capturing,
            timestamp_ms: 42,
        });
        assert_eq!(ipc.capture_phase, "capturing");
        assert_eq!(ipc.session_phase, CaptureSessionPhase::Active);
    }

    #[test]
    fn get_delegates_to_service() {
        let service = MockCaptureSessionService::with_state(sample_snapshot());
        let state = get_capture_session_state_impl(&service);
        assert_eq!(state.capture_phase, "idle");
        assert_eq!(state.session_phase, CaptureSessionPhase::Idle);
    }

    #[test]
    fn start_delegates_to_service() {
        let service = MockCaptureSessionService::with_state(sample_snapshot());
        let state = start_capture_session_impl(&service).expect("start");
        assert_eq!(state.session_phase, CaptureSessionPhase::Active);
    }

    #[test]
    fn start_maps_transition_busy_error() {
        let service = MockCaptureSessionService::with_start_error(
            sample_snapshot(),
            CaptureSessionError::transition_busy(),
        );
        let err = start_capture_session_impl(&service).expect_err("busy");
        assert_eq!(err.code, "TRANSITION_BUSY");
    }

    #[test]
    fn invoke_error_serializes_contract_shape() {
        let err = CaptureSessionInvokeError::from_service_error(CaptureSessionError {
            code: CaptureSessionErrorCode::TransitionBusy,
            message_ja: "キャプチャセッションの開始を処理中です。".to_string(),
            action_ja: "処理が完了するまでお待ちください。".to_string(),
        });
        assert_invoke_error_serializes_contract_shape(&err, "TRANSITION_BUSY");
    }

    #[test]
    fn state_json_uses_contract_field_names() {
        let state = snapshot_to_ipc_state(sample_snapshot());
        let value = serde_json::to_value(&state).expect("serialize");
        let obj = value.as_object().expect("object");
        for key in [
            "session_phase",
            "transition_busy",
            "capture_phase",
            "timestamp_ms",
        ] {
            assert!(obj.contains_key(key), "missing contract field {key}");
        }
        assert!(
            !obj.contains_key("stop_flush_in_progress"),
            "stop_flush_in_progress must not be in contract state"
        );
    }

    #[test]
    fn emitter_records_state_changed_payload() {
        let emitter = RecordingCaptureSessionEventEmitter::new();
        let snapshot = sample_snapshot();
        emitter.emit_state_changed(&snapshot);
        let events = emitter.state_changes();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].state.capture_phase, "idle");
    }
}
