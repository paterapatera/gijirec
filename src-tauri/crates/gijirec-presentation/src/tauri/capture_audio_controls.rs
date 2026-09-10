//! Testable capture audio controls command logic and Tauri event emission.
//!
//! Thin `#[tauri::command]` wrappers live in the host `commands` module (task 6.1).

use crate::application::capture_audio_controls::{
    CaptureAudioControlsError, CaptureAudioControlsEvents, CaptureAudioControlsPatch,
    CaptureAudioControlsService,
};
use crate::tauri::events::{ERROR_EVENT, build_error_payload};
use gijirec_domain::audio::{CaptureAudioControls, CaptureError};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};

/// Tauri event name for controls changes.
pub const CONTROLS_CHANGED_EVENT: &str = "capture-audio-controls://controls-changed";

/// Latest ingest-level meter snapshot (null until emitter wired in task 9.1).
pub type IngestLevelSnapshotCache = Arc<Mutex<Option<IngestLevelSnapshot>>>;

/// Invoke error payload per `docs/contracts/capture-audio-controls.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureAudioControlsInvokeError {
    pub code: String,
    pub message_ja: String,
    pub action_ja: String,
}

impl CaptureAudioControlsInvokeError {
    pub fn from_service_error(err: CaptureAudioControlsError) -> Self {
        Self {
            code: err.code.as_str().to_string(),
            message_ja: err.message_ja,
            action_ja: err.action_ja,
        }
    }
}

/// Controls-only IPC response shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureAudioControlsResponse {
    pub mic_ingest_enabled: bool,
    pub manual_ingest_gain: f32,
    pub gain_user_adjusted: bool,
}

impl From<CaptureAudioControls> for CaptureAudioControlsResponse {
    fn from(controls: CaptureAudioControls) -> Self {
        Self {
            mic_ingest_enabled: controls.mic_ingest_enabled,
            manual_ingest_gain: controls.manual_ingest_gain,
            gain_user_adjusted: controls.gain_user_adjusted,
        }
    }
}

/// dBFS meter snapshot per `docs/contracts/capture-audio-controls.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestLevelSnapshot {
    pub level_dbfs: f32,
    pub timestamp_ms: u64,
}

/// Full get/set response including optional ingest meter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureAudioControlsStateResponse {
    pub controls: CaptureAudioControlsResponse,
    pub ingest_level: Option<IngestLevelSnapshot>,
}

/// Partial update request (only sent fields are applied).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct CaptureAudioControlsPatchRequest {
    pub mic_ingest_enabled: Option<bool>,
    pub manual_ingest_gain: Option<f32>,
    pub gain_user_adjusted: Option<bool>,
}

impl From<CaptureAudioControlsPatchRequest> for CaptureAudioControlsPatch {
    fn from(request: CaptureAudioControlsPatchRequest) -> CaptureAudioControlsPatch {
        CaptureAudioControlsPatch {
            mic_ingest_enabled: request.mic_ingest_enabled,
            manual_ingest_gain: request.manual_ingest_gain,
            gain_user_adjusted: request.gain_user_adjusted,
        }
    }
}

/// Payload per `docs/contracts/capture-audio-controls.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureAudioControlsChangedPayload {
    pub controls: CaptureAudioControls,
    pub timestamp_ms: u64,
}

pub fn build_controls_changed_payload(
    controls: &CaptureAudioControls,
    timestamp_ms: u64,
) -> CaptureAudioControlsChangedPayload {
    CaptureAudioControlsChangedPayload {
        controls: *controls,
        timestamp_ms,
    }
}

fn build_state_response(
    controls: CaptureAudioControls,
    ingest_level_cache: &IngestLevelSnapshotCache,
) -> CaptureAudioControlsStateResponse {
    let ingest_level = ingest_level_cache
        .lock()
        .expect("ingest level cache lock poisoned")
        .clone();
    CaptureAudioControlsStateResponse {
        controls: CaptureAudioControlsResponse::from(controls),
        ingest_level,
    }
}

/// Production emitter backed by [`AppHandle`], implementing [`CaptureAudioControlsEvents`].
pub struct TauriCaptureAudioControlsEventEmitter<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriCaptureAudioControlsEventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> CaptureAudioControlsEvents for TauriCaptureAudioControlsEventEmitter<R> {
    fn emit_controls_changed(&self, controls: &CaptureAudioControls) {
        let payload = build_controls_changed_payload(controls, current_timestamp_ms());
        let _ = self.app.emit(CONTROLS_CHANGED_EVENT, payload);
    }

    fn emit_capture_error(&self, error: CaptureError) {
        let payload = build_error_payload(error);
        let _ = self.app.emit(ERROR_EVENT, payload);
    }
}

/// In-memory recorder for unit tests.
#[derive(Debug, Default, Clone)]
pub struct RecordingCaptureAudioControlsEventEmitter {
    controls: Arc<Mutex<Vec<CaptureAudioControlsChangedPayload>>>,
    errors: Arc<Mutex<Vec<CaptureError>>>,
}

impl RecordingCaptureAudioControlsEventEmitter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn controls_changed(&self) -> Vec<CaptureAudioControlsChangedPayload> {
        self.controls.lock().expect("lock").clone()
    }

    pub fn capture_errors(&self) -> Vec<CaptureError> {
        self.errors.lock().expect("lock").clone()
    }
}

impl CaptureAudioControlsEvents for RecordingCaptureAudioControlsEventEmitter {
    fn emit_controls_changed(&self, controls: &CaptureAudioControls) {
        self.controls
            .lock()
            .expect("lock")
            .push(build_controls_changed_payload(
                controls,
                current_timestamp_ms(),
            ));
    }

    fn emit_capture_error(&self, error: CaptureError) {
        self.errors.lock().expect("lock").push(error);
    }
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Returns current session capture audio controls and optional ingest meter.
pub fn get_capture_audio_controls_impl(
    service: &dyn CaptureAudioControlsService,
    ingest_level_cache: &IngestLevelSnapshotCache,
) -> CaptureAudioControlsStateResponse {
    let state = service.get_state();
    build_state_response(state.controls, ingest_level_cache)
}

/// Applies a partial capture audio controls update.
pub fn set_capture_audio_controls_impl(
    service: &dyn CaptureAudioControlsService,
    ingest_level_cache: &IngestLevelSnapshotCache,
    patch: CaptureAudioControlsPatchRequest,
) -> Result<CaptureAudioControlsStateResponse, CaptureAudioControlsInvokeError> {
    let state = service
        .apply_partial(patch.into())
        .map_err(CaptureAudioControlsInvokeError::from_service_error)?;
    Ok(build_state_response(state.controls, ingest_level_cache))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::capture_audio_controls::{
        CaptureAudioControlsErrorCode, CaptureAudioControlsStore, CapturePhasePort,
        DefaultCaptureAudioControlsService, NoopCaptureAudioControlsApplyPort,
        NoopIngestSourcePort,
    };
    use gijirec_domain::audio::{
        CapturePhase, DEFAULT_INGEST_GAIN, MAX_INGEST_GAIN, MIN_INGEST_GAIN,
    };

    struct MockPhasePort {
        phase: CapturePhase,
    }

    impl CapturePhasePort for MockPhasePort {
        fn capture_phase(&self) -> CapturePhase {
            self.phase
        }
    }

    fn test_service(
        events: RecordingCaptureAudioControlsEventEmitter,
    ) -> DefaultCaptureAudioControlsService<
        MockPhasePort,
        NoopCaptureAudioControlsApplyPort,
        NoopIngestSourcePort,
        RecordingCaptureAudioControlsEventEmitter,
    > {
        DefaultCaptureAudioControlsService::new(
            CaptureAudioControlsStore::new(),
            MockPhasePort {
                phase: CapturePhase::Idle,
            },
            NoopCaptureAudioControlsApplyPort,
            NoopIngestSourcePort,
            events,
        )
    }

    fn empty_cache() -> IngestLevelSnapshotCache {
        Arc::new(Mutex::new(None))
    }

    #[test]
    fn get_returns_contract_defaults_with_null_ingest_level() {
        let service = test_service(RecordingCaptureAudioControlsEventEmitter::new());
        let cache = empty_cache();

        let response = get_capture_audio_controls_impl(&service, &cache);

        assert!(response.controls.mic_ingest_enabled);
        assert_eq!(response.controls.manual_ingest_gain, DEFAULT_INGEST_GAIN);
        assert!(!response.controls.gain_user_adjusted);
        assert!(response.ingest_level.is_none());
    }

    #[test]
    fn set_applies_partial_mic_toggle() {
        let emitter = RecordingCaptureAudioControlsEventEmitter::new();
        let service = test_service(emitter.clone());
        let cache = empty_cache();

        let response = set_capture_audio_controls_impl(
            &service,
            &cache,
            CaptureAudioControlsPatchRequest {
                mic_ingest_enabled: Some(false),
                ..Default::default()
            },
        )
        .expect("set");

        assert!(!response.controls.mic_ingest_enabled);
        assert_eq!(response.controls.manual_ingest_gain, DEFAULT_INGEST_GAIN);
        assert_eq!(
            get_capture_audio_controls_impl(&service, &cache).controls,
            response.controls
        );
    }

    #[test]
    fn set_maps_invalid_gain_error() {
        let service = test_service(RecordingCaptureAudioControlsEventEmitter::new());
        let cache = empty_cache();

        let err = set_capture_audio_controls_impl(
            &service,
            &cache,
            CaptureAudioControlsPatchRequest {
                manual_ingest_gain: Some(MAX_INGEST_GAIN + 1.0),
                ..Default::default()
            },
        )
        .expect_err("invalid gain");

        assert_eq!(err.code, "INVALID_GAIN");
        assert_eq!(err.message_ja, "ゲインの値が不正です");
        assert_eq!(
            err.action_ja,
            "スライダーを中央付近に戻して再度お試しください"
        );
    }

    #[test]
    fn set_maps_internal_error_code_shape() {
        let err = CaptureAudioControlsInvokeError::from_service_error(
            CaptureAudioControlsError::internal("store failed"),
        );
        assert_eq!(err.code, "INTERNAL");
        assert_eq!(err.message_ja, "音声設定の更新に失敗しました");
        assert_eq!(err.action_ja, "アプリを再起動してください");
    }

    #[test]
    fn set_emits_controls_changed_on_success() {
        let emitter = RecordingCaptureAudioControlsEventEmitter::new();
        let service = test_service(emitter.clone());
        let cache = empty_cache();

        let response = set_capture_audio_controls_impl(
            &service,
            &cache,
            CaptureAudioControlsPatchRequest {
                manual_ingest_gain: Some(2.0),
                ..Default::default()
            },
        )
        .expect("set");

        let events = emitter.controls_changed();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].controls.manual_ingest_gain,
            response.controls.manual_ingest_gain
        );
        assert!(events[0].timestamp_ms > 0);
        assert!(response.controls.gain_user_adjusted);
    }

    #[test]
    fn get_includes_cached_ingest_level_when_present() {
        let service = test_service(RecordingCaptureAudioControlsEventEmitter::new());
        let cache = Arc::new(Mutex::new(Some(IngestLevelSnapshot {
            level_dbfs: -18.5,
            timestamp_ms: 42_000,
        })));

        let response = get_capture_audio_controls_impl(&service, &cache);

        let level = response.ingest_level.expect("level");
        assert_eq!(level.level_dbfs, -18.5);
        assert_eq!(level.timestamp_ms, 42_000);
    }

    #[test]
    fn invoke_error_serializes_contract_shape() {
        let err = CaptureAudioControlsInvokeError::from_service_error(CaptureAudioControlsError {
            code: CaptureAudioControlsErrorCode::InvalidGain,
            message_ja: "ゲインの値が不正です".to_string(),
            action_ja: "スライダーを中央付近に戻して再度お試しください".to_string(),
        });
        let json = serde_json::to_value(&err).expect("serialize");
        let obj = json.as_object().expect("object");
        assert_eq!(
            obj.get("code").and_then(|v| v.as_str()),
            Some("INVALID_GAIN")
        );
        assert!(obj.get("message_ja").and_then(|v| v.as_str()).is_some());
        assert!(obj.get("action_ja").and_then(|v| v.as_str()).is_some());
        assert_eq!(obj.len(), 3);
    }

    #[test]
    fn patch_request_accepts_boundary_gain() {
        let emitter = RecordingCaptureAudioControlsEventEmitter::new();
        let service = test_service(emitter);
        let cache = empty_cache();

        for gain in [MIN_INGEST_GAIN, MAX_INGEST_GAIN] {
            let response = set_capture_audio_controls_impl(
                &service,
                &cache,
                CaptureAudioControlsPatchRequest {
                    manual_ingest_gain: Some(gain),
                    ..Default::default()
                },
            )
            .expect("valid gain");
            assert_eq!(response.controls.manual_ingest_gain, gain);
        }
    }
}
