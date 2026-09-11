//! Capture audio controls validation, store updates, and live apply coordination.

use gijirec_domain::audio::{
    CaptureAudioControls, CaptureError, CapturePhase, validate_manual_ingest_gain,
};

use super::store::CaptureAudioControlsStore;

/// Invoke error codes for capture audio controls commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureAudioControlsErrorCode {
    InvalidGain,
    Internal,
}

impl CaptureAudioControlsErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidGain => "INVALID_GAIN",
            Self::Internal => "INTERNAL",
        }
    }
}

/// Application error for capture audio controls operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureAudioControlsError {
    pub code: CaptureAudioControlsErrorCode,
    pub message_ja: String,
    pub action_ja: String,
}

crate::user_facing_error::impl_message_ja_error_display!(CaptureAudioControlsError);

impl CaptureAudioControlsError {
    pub fn invalid_gain() -> Self {
        Self {
            code: CaptureAudioControlsErrorCode::InvalidGain,
            message_ja: "ゲインの値が不正です".to_string(),
            action_ja: "スライダーを中央付近に戻して再度お試しください".to_string(),
        }
    }

    pub fn internal(_detail: impl Into<String>) -> Self {
        Self {
            code: CaptureAudioControlsErrorCode::Internal,
            message_ja: "音声設定の更新に失敗しました".to_string(),
            action_ja: "アプリを再起動してください".to_string(),
        }
    }
}

/// Partial update with explicit optional fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CaptureAudioControlsPatch {
    pub mic_ingest_enabled: Option<bool>,
    pub manual_ingest_gain: Option<f32>,
    pub gain_user_adjusted: Option<bool>,
}

/// Service read model. Ingest level metering arrives in a later major.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureAudioControlsState {
    pub controls: CaptureAudioControls,
}

/// Reads current capture phase for live-apply gating.
pub trait CapturePhasePort: Send + Sync {
    fn capture_phase(&self) -> CapturePhase;
}

pub struct NoopCapturePhasePort;

impl CapturePhasePort for NoopCapturePhasePort {
    fn capture_phase(&self) -> CapturePhase {
        CapturePhase::Idle
    }
}

/// Live apply hook for mic gate and ingest gain (called only while capturing).
pub trait CaptureAudioControlsApplyPort: Send + Sync {
    fn set_mic_ingest_enabled(&self, enabled: bool);
    fn set_ingest_gain_multiplier(&self, gain: f32);
}

pub struct NoopCaptureAudioControlsApplyPort;

impl CaptureAudioControlsApplyPort for NoopCaptureAudioControlsApplyPort {
    fn set_mic_ingest_enabled(&self, _enabled: bool) {}
    fn set_ingest_gain_multiplier(&self, _gain: f32) {}
}

/// Checks whether ingest can proceed when mic ingest is disabled.
pub trait IngestSourcePort: Send + Sync {
    fn has_ingestable_audio_source(&self, mic_enabled: bool) -> bool;
}

pub struct NoopIngestSourcePort;

impl IngestSourcePort for NoopIngestSourcePort {
    fn has_ingestable_audio_source(&self, _mic_enabled: bool) -> bool {
        true
    }
}

/// Emits controls-changed and capture error notifications.
pub trait CaptureAudioControlsEvents: Send + Sync {
    fn emit_controls_changed(&self, controls: &CaptureAudioControls);
    fn emit_capture_error(&self, error: CaptureError);
}

pub struct NoopCaptureAudioControlsEvents;

impl CaptureAudioControlsEvents for NoopCaptureAudioControlsEvents {
    fn emit_controls_changed(&self, _controls: &CaptureAudioControls) {}
    fn emit_capture_error(&self, _error: CaptureError) {}
}

/// Service API per design D-CaptureAudioControlsService.
pub trait CaptureAudioControlsService: Send + Sync {
    fn get_state(&self) -> CaptureAudioControlsState;
    fn apply_partial(
        &self,
        patch: CaptureAudioControlsPatch,
    ) -> Result<CaptureAudioControlsState, CaptureAudioControlsError>;
}

/// Default capture audio controls service with injected ports.
pub struct DefaultCaptureAudioControlsService<P, A, I, E> {
    store: CaptureAudioControlsStore,
    phase_port: P,
    apply_port: A,
    ingest_source_port: I,
    events: E,
}

impl<P, A, I, E> DefaultCaptureAudioControlsService<P, A, I, E> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: CaptureAudioControlsStore,
        phase_port: P,
        apply_port: A,
        ingest_source_port: I,
        events: E,
    ) -> Self {
        Self {
            store,
            phase_port,
            apply_port,
            ingest_source_port,
            events,
        }
    }
}

impl<P, A, I, E> DefaultCaptureAudioControlsService<P, A, I, E>
where
    P: CapturePhasePort,
    A: CaptureAudioControlsApplyPort,
    I: IngestSourcePort,
    E: CaptureAudioControlsEvents,
{
    fn apply_live_if_capturing(
        &self,
        controls: &CaptureAudioControls,
    ) -> Result<(), CaptureAudioControlsError> {
        if self.phase_port.capture_phase() != CapturePhase::Capturing {
            return Ok(());
        }

        self.apply_port
            .set_mic_ingest_enabled(controls.mic_ingest_enabled);
        self.apply_port
            .set_ingest_gain_multiplier(controls.manual_ingest_gain);

        if !controls.mic_ingest_enabled
            && !self
                .ingest_source_port
                .has_ingestable_audio_source(controls.mic_ingest_enabled)
        {
            self.events
                .emit_capture_error(CaptureError::TranscribeIngestNoAudioSource);
        }

        Ok(())
    }
}

impl<P, A, I, E> CaptureAudioControlsService for DefaultCaptureAudioControlsService<P, A, I, E>
where
    P: CapturePhasePort,
    A: CaptureAudioControlsApplyPort,
    I: IngestSourcePort,
    E: CaptureAudioControlsEvents,
{
    fn get_state(&self) -> CaptureAudioControlsState {
        CaptureAudioControlsState {
            controls: self.store.get_controls(),
        }
    }

    fn apply_partial(
        &self,
        patch: CaptureAudioControlsPatch,
    ) -> Result<CaptureAudioControlsState, CaptureAudioControlsError> {
        let mut next = self.store.get_controls();

        if let Some(gain) = patch.manual_ingest_gain {
            validate_manual_ingest_gain(gain)
                .map_err(|_| CaptureAudioControlsError::invalid_gain())?;
            next.manual_ingest_gain = gain;
            next.gain_user_adjusted = true;
        }

        if let Some(mic_enabled) = patch.mic_ingest_enabled {
            next.mic_ingest_enabled = mic_enabled;
        }

        if let Some(gain_user_adjusted) = patch.gain_user_adjusted {
            next.gain_user_adjusted = gain_user_adjusted;
        }

        self.store.update(next);
        self.events.emit_controls_changed(&next);
        self.apply_live_if_capturing(&next)?;

        Ok(self.get_state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::{DEFAULT_INGEST_GAIN, MAX_INGEST_GAIN, MIN_INGEST_GAIN};
    use std::sync::{Arc, Mutex};

    struct MockPhasePort {
        phase: CapturePhase,
    }

    impl CapturePhasePort for MockPhasePort {
        fn capture_phase(&self) -> CapturePhase {
            self.phase
        }
    }

    struct RecordingApplyPort {
        mic_enabled: Arc<Mutex<Vec<bool>>>,
        gains: Arc<Mutex<Vec<f32>>>,
    }

    impl CaptureAudioControlsApplyPort for RecordingApplyPort {
        fn set_mic_ingest_enabled(&self, enabled: bool) {
            self.mic_enabled.lock().expect("lock").push(enabled);
        }

        fn set_ingest_gain_multiplier(&self, gain: f32) {
            self.gains.lock().expect("lock").push(gain);
        }
    }

    struct MockIngestSourcePort {
        has_source: bool,
    }

    impl IngestSourcePort for MockIngestSourcePort {
        fn has_ingestable_audio_source(&self, _mic_enabled: bool) -> bool {
            self.has_source
        }
    }

    struct MockEvents {
        controls: Arc<Mutex<Vec<CaptureAudioControls>>>,
        errors: Arc<Mutex<Vec<CaptureError>>>,
    }

    impl CaptureAudioControlsEvents for MockEvents {
        fn emit_controls_changed(&self, controls: &CaptureAudioControls) {
            self.controls.lock().expect("lock").push(*controls);
        }

        fn emit_capture_error(&self, error: CaptureError) {
            self.errors.lock().expect("lock").push(error);
        }
    }

    #[allow(clippy::type_complexity)]
    fn service_with_ports(
        phase: CapturePhase,
        has_ingest_source: bool,
    ) -> (
        DefaultCaptureAudioControlsService<
            MockPhasePort,
            RecordingApplyPort,
            MockIngestSourcePort,
            MockEvents,
        >,
        Arc<Mutex<Vec<bool>>>,
        Arc<Mutex<Vec<f32>>>,
        Arc<Mutex<Vec<CaptureAudioControls>>>,
        Arc<Mutex<Vec<CaptureError>>>,
    ) {
        let mic_calls = Arc::new(Mutex::new(Vec::new()));
        let gain_calls = Arc::new(Mutex::new(Vec::new()));
        let control_events = Arc::new(Mutex::new(Vec::new()));
        let error_events = Arc::new(Mutex::new(Vec::new()));

        let service = DefaultCaptureAudioControlsService::new(
            CaptureAudioControlsStore::new(),
            MockPhasePort { phase },
            RecordingApplyPort {
                mic_enabled: Arc::clone(&mic_calls),
                gains: Arc::clone(&gain_calls),
            },
            MockIngestSourcePort {
                has_source: has_ingest_source,
            },
            MockEvents {
                controls: Arc::clone(&control_events),
                errors: Arc::clone(&error_events),
            },
        );

        (service, mic_calls, gain_calls, control_events, error_events)
    }

    #[test]
    fn get_state_returns_default_gain_1_25() {
        let (service, _, _, _, _) = service_with_ports(CapturePhase::Idle, true);
        let state = service.get_state();

        assert_eq!(state.controls.manual_ingest_gain, DEFAULT_INGEST_GAIN);
        assert!(!state.controls.gain_user_adjusted);
    }

    #[test]
    fn apply_rejects_out_of_range_gain_with_invalid_gain() {
        let (service, _, _, _, _) = service_with_ports(CapturePhase::Idle, true);

        let err = service
            .apply_partial(CaptureAudioControlsPatch {
                manual_ingest_gain: Some(MIN_INGEST_GAIN - 0.01),
                ..Default::default()
            })
            .expect_err("below min");
        assert_eq!(err.code, CaptureAudioControlsErrorCode::InvalidGain);

        let err = service
            .apply_partial(CaptureAudioControlsPatch {
                manual_ingest_gain: Some(MAX_INGEST_GAIN + 0.01),
                ..Default::default()
            })
            .expect_err("above max");
        assert_eq!(err.code, CaptureAudioControlsErrorCode::InvalidGain);
    }

    #[test]
    fn apply_rejects_non_finite_gain_with_invalid_gain() {
        let (service, _, _, _, _) = service_with_ports(CapturePhase::Idle, true);

        for gain in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let err = service
                .apply_partial(CaptureAudioControlsPatch {
                    manual_ingest_gain: Some(gain),
                    ..Default::default()
                })
                .expect_err("non-finite");
            assert_eq!(err.code, CaptureAudioControlsErrorCode::InvalidGain);
        }
    }

    #[test]
    fn apply_accepts_boundary_gains() {
        let (service, _, _, _, _) = service_with_ports(CapturePhase::Idle, true);

        for gain in [MIN_INGEST_GAIN, MAX_INGEST_GAIN, DEFAULT_INGEST_GAIN] {
            let state = service
                .apply_partial(CaptureAudioControlsPatch {
                    manual_ingest_gain: Some(gain),
                    ..Default::default()
                })
                .expect("valid gain");
            assert_eq!(state.controls.manual_ingest_gain, gain);
        }
    }

    #[test]
    fn sending_gain_sets_gain_user_adjusted_true() {
        let (service, _, _, _, _) = service_with_ports(CapturePhase::Idle, true);

        let state = service
            .apply_partial(CaptureAudioControlsPatch {
                manual_ingest_gain: Some(2.0),
                ..Default::default()
            })
            .expect("apply gain");

        assert!(state.controls.gain_user_adjusted);
    }

    #[test]
    fn explicit_gain_user_adjusted_false_resets_flag() {
        let (service, _, _, _, _) = service_with_ports(CapturePhase::Idle, true);

        service
            .apply_partial(CaptureAudioControlsPatch {
                manual_ingest_gain: Some(2.0),
                ..Default::default()
            })
            .expect("apply gain");

        let state = service
            .apply_partial(CaptureAudioControlsPatch {
                gain_user_adjusted: Some(false),
                ..Default::default()
            })
            .expect("reset flag");

        assert!(!state.controls.gain_user_adjusted);
        assert_eq!(state.controls.manual_ingest_gain, 2.0);
    }

    #[test]
    fn non_capturing_updates_store_without_live_apply() {
        let (service, mic_calls, gain_calls, control_events, _) =
            service_with_ports(CapturePhase::Idle, true);

        let state = service
            .apply_partial(CaptureAudioControlsPatch {
                mic_ingest_enabled: Some(false),
                manual_ingest_gain: Some(3.0),
                ..Default::default()
            })
            .expect("apply");

        assert!(!state.controls.mic_ingest_enabled);
        assert_eq!(state.controls.manual_ingest_gain, 3.0);
        assert!(mic_calls.lock().expect("lock").is_empty());
        assert!(gain_calls.lock().expect("lock").is_empty());
        assert_eq!(control_events.lock().expect("lock").len(), 1);
    }

    #[test]
    fn capturing_live_applies_mic_and_gain() {
        let (service, mic_calls, gain_calls, _, _) =
            service_with_ports(CapturePhase::Capturing, true);

        service
            .apply_partial(CaptureAudioControlsPatch {
                mic_ingest_enabled: Some(false),
                manual_ingest_gain: Some(2.5),
                ..Default::default()
            })
            .expect("apply");

        assert_eq!(mic_calls.lock().expect("lock").as_slice(), &[false]);
        assert_eq!(gain_calls.lock().expect("lock").as_slice(), &[2.5]);
    }

    #[test]
    fn mic_off_without_ingest_source_emits_transcribe_ingest_no_audio_source() {
        let (service, _, _, _, error_events) = service_with_ports(CapturePhase::Capturing, false);

        service
            .apply_partial(CaptureAudioControlsPatch {
                mic_ingest_enabled: Some(false),
                ..Default::default()
            })
            .expect("apply");

        let errors = error_events.lock().expect("lock");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], CaptureError::TranscribeIngestNoAudioSource);
    }

    #[test]
    fn store_retains_controls_after_simulated_recapture() {
        let store = CaptureAudioControlsStore::new();
        let (service, _, _, _, _) = (
            DefaultCaptureAudioControlsService::new(
                store,
                MockPhasePort {
                    phase: CapturePhase::Capturing,
                },
                RecordingApplyPort {
                    mic_enabled: Arc::new(Mutex::new(Vec::new())),
                    gains: Arc::new(Mutex::new(Vec::new())),
                },
                MockIngestSourcePort { has_source: true },
                NoopCaptureAudioControlsEvents,
            ),
            Arc::new(Mutex::new(Vec::<bool>::new())),
            Arc::new(Mutex::new(Vec::<f32>::new())),
            Arc::new(Mutex::new(Vec::<CaptureAudioControls>::new())),
            Arc::new(Mutex::new(Vec::<CaptureError>::new())),
        );

        let expected = service
            .apply_partial(CaptureAudioControlsPatch {
                mic_ingest_enabled: Some(false),
                manual_ingest_gain: Some(3.75),
                ..Default::default()
            })
            .expect("apply")
            .controls;

        // Simulated recapture: phase may change but store is untouched.
        assert_eq!(service.get_state().controls, expected);
    }

    #[test]
    fn invalid_gain_error_messages_match_contract() {
        let err = CaptureAudioControlsError::invalid_gain();
        assert_eq!(err.message_ja, "ゲインの値が不正です");
        assert_eq!(
            err.action_ja,
            "スライダーを中央付近に戻して再度お試しください"
        );
    }
}
