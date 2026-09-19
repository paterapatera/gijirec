//! Integration tests for capture-session-toggle tasks 12.1–12.3.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gijirec_domain::audio::{AudioDeviceId, CaptureError, CapturePhase, DeviceSelection};
use gijirec_domain::capture_session::CaptureSessionPhase;
use gijirec_domain::transcribe::TranscribePhase;
use gijirec_presentation::application::capture::orchestrator::CaptureOrchestrator;
use gijirec_presentation::application::capture_session::{
    CaptureSessionClock, CaptureSessionEvents, CaptureSessionPlatform,
    CaptureSessionProcessingHook, CaptureSessionService, CaptureSessionServiceApi,
    NoopCaptureSessionObservability,
};
use gijirec_presentation::application::device_selection::DeviceSelectionService;
use gijirec_presentation::application::editor::SettingsService;
use gijirec_presentation::application::transcribe::model_orchestrator::ModelOrchestrator;
use gijirec_presentation::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use gijirec_presentation::domain::audio::CapturePhase as PresentationCapturePhase;
use gijirec_presentation::domain::editor::SaveTranscriptSessionRequest;
use gijirec_presentation::editor::{save_transcript_session_impl, set_editor_settings_impl};
use gijirec_presentation::tauri::device_selection::set_device_selection_impl;
use gijirec_presentation::transcribe::TranscribeLifecycleHook;
use gijirec_presentation::transcribe::test_support::{
    InjectableMockStore, MockSequenceDownloader, NoopTranscribeWorkerPort, NoopWhisperContextPort,
    RecordingTranscribeEventEmitter, assert_counting_batch_pipeline_emits_two_contiguous_blocks,
    create_temp_model_file,
};

struct FixedPlatform {
    supported: bool,
}

impl CaptureSessionPlatform for FixedPlatform {
    fn is_capture_supported(&self) -> bool {
        self.supported
    }
}

struct FixedClock {
    now_ms: u64,
}

impl CaptureSessionClock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

struct RecordingSessionEvents {
    states: Mutex<Vec<gijirec_presentation::application::capture_session::CaptureSessionSnapshot>>,
}

impl RecordingSessionEvents {
    fn new() -> Self {
        Self {
            states: Mutex::new(Vec::new()),
        }
    }
}

impl CaptureSessionEvents for RecordingSessionEvents {
    fn emit_state_changed(
        &self,
        state: &gijirec_presentation::application::capture_session::CaptureSessionSnapshot,
    ) {
        self.states.lock().expect("lock").push(state.clone());
    }
}

struct NoopProcessing;

impl CaptureSessionProcessingHook for NoopProcessing {
    fn on_capture_started(&self) {}
    fn on_capture_stopping(&self) {}
}

/* jscpd:ignore-start — integration mock orchestrator; mirrors lifecycle TrackingOrchestrator */
struct SessionOrchestrator {
    phase: CapturePhase,
    start_with_selection_calls: AtomicUsize,
    restart_with_selection_calls: AtomicUsize,
}

impl SessionOrchestrator {
    fn idle() -> Self {
        Self {
            phase: CapturePhase::Idle,
            start_with_selection_calls: AtomicUsize::new(0),
            restart_with_selection_calls: AtomicUsize::new(0),
        }
    }
}

impl CaptureOrchestrator for SessionOrchestrator {
    fn start(&mut self) -> Result<(), CaptureError> {
        self.start_with_selection(&DeviceSelection::default())
    }

    fn start_with_selection(&mut self, _selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.start_with_selection_calls
            .fetch_add(1, Ordering::SeqCst);
        self.phase = CapturePhase::Capturing;
        Ok(())
    }

    fn restart_with_selection(&mut self, _selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.restart_with_selection_calls
            .fetch_add(1, Ordering::SeqCst);
        self.phase = CapturePhase::Capturing;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.phase = CapturePhase::Idle;
        Ok(())
    }

    fn phase(&self) -> CapturePhase {
        self.phase
    }

    fn on_device_disconnected(&mut self) -> Result<(), CaptureError> {
        Ok(())
    }
}
/* jscpd:ignore-end */

struct MutableDeviceSelection {
    selection: Mutex<DeviceSelection>,
}

impl DeviceSelectionService for MutableDeviceSelection {
    fn list_devices(
        &self,
    ) -> Result<
        gijirec_domain::audio::AudioDeviceList,
        gijirec_presentation::application::device_selection::DeviceSelectionError,
    > {
        Ok(gijirec_domain::audio::AudioDeviceList::default())
    }

    fn get_selection(&self) -> DeviceSelection {
        self.selection.lock().expect("lock").clone()
    }

    fn set_selection(
        &self,
        selection: DeviceSelection,
    ) -> Result<
        DeviceSelection,
        gijirec_presentation::application::device_selection::DeviceSelectionError,
    > {
        *self.selection.lock().expect("lock") = selection.clone();
        Ok(selection)
    }

    fn set_ui_visible(&self, _visible: bool) {}
}

fn build_session_service() -> (
    Arc<CaptureSessionService>,
    Arc<Mutex<SessionOrchestrator>>,
    Arc<MutableDeviceSelection>,
) {
    let orchestrator = Arc::new(Mutex::new(SessionOrchestrator::idle()));
    let selection = Arc::new(MutableDeviceSelection {
        selection: Mutex::new(DeviceSelection::default()),
    });
    let service = Arc::new(CaptureSessionService::new(
        orchestrator.clone(),
        selection.clone(),
        Arc::new(FixedPlatform { supported: true }),
        Arc::new(NoopProcessing),
        Arc::new(RecordingSessionEvents::new()),
        Arc::new(FixedClock { now_ms: 42 }),
        Arc::new(NoopCaptureSessionObservability),
    ));
    (service, orchestrator, selection)
}

fn device_id(label: &str) -> AudioDeviceId {
    AudioDeviceId::new(label.to_string()).expect("device id")
}

/// Task 12.1 / req 3.1–3.2: user session start → capturing → 30s batch transcription continues.
#[test]
fn integration_task_12_1_session_start_capture_batch_continue() {
    let (service, orchestrator, _) = build_session_service();
    service.start().expect("session start");
    let state = service.get_state();
    assert_eq!(state.session_phase, CaptureSessionPhase::Active);
    assert_eq!(state.capture_phase, CapturePhase::Capturing);
    assert_eq!(
        orchestrator
            .lock()
            .expect("lock")
            .start_with_selection_calls
            .load(Ordering::SeqCst),
        1
    );

    assert_counting_batch_pipeline_emits_two_contiguous_blocks();
}

/// Task 12.2 / req 4.3: device selection changes do not start a session; active session stays stable.
#[test]
fn integration_task_12_2_device_selection_independent_of_session_start() {
    let (service, orchestrator, selection_service) = build_session_service();

    set_device_selection_impl(
        selection_service.as_ref(),
        DeviceSelection::new(Some(device_id("mic-a")), None),
    )
    .expect("set selection while idle");

    assert_eq!(
        orchestrator
            .lock()
            .expect("lock")
            .start_with_selection_calls
            .load(Ordering::SeqCst),
        0,
        "device selection alone must not start capture"
    );

    service.start().expect("user session start");
    assert_eq!(
        orchestrator.lock().expect("lock").phase(),
        CapturePhase::Capturing
    );

    set_device_selection_impl(
        selection_service.as_ref(),
        DeviceSelection::new(Some(device_id("mic-b")), None),
    )
    .expect("change selection during session");

    assert_eq!(
        orchestrator
            .lock()
            .expect("lock")
            .restart_with_selection_calls
            .load(Ordering::SeqCst),
        0,
        "selection service alone must not restart capture; restart is orchestrator command path"
    );

    let second = service.start().expect("idempotent session start");
    assert_eq!(second.session_phase, CaptureSessionPhase::Active);
    assert_eq!(
        orchestrator
            .lock()
            .expect("lock")
            .start_with_selection_calls
            .load(Ordering::SeqCst),
        1,
        "second session start must remain idempotent"
    );
}

/// Task 12.3 / req 6.1: save succeeds regardless of transcribe phase (no save blocking).
#[test]
fn integration_task_12_3_save_transcript_session_while_transcribing() {
    let data_dir =
        std::env::temp_dir().join(format!("gijirec-cst-save-12-3-{}", std::process::id()));
    std::fs::create_dir_all(&data_dir).expect("temp dir");
    let save_dir =
        std::env::temp_dir().join(format!("gijirec-cst-save-out-12-3-{}", std::process::id()));
    std::fs::create_dir_all(&save_dir).expect("save dir");

    let service = SettingsService::new(data_dir);
    set_editor_settings_impl(
        &service,
        Some(Some(save_dir.to_string_lossy().into_owned())),
        None,
    )
    .expect("seed save directory");

    let model_path = create_temp_model_file();
    let transcribe_orch: Arc<Mutex<dyn TranscribeOrchestrator>> =
        Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
            Arc::new(Mutex::new(NoopTranscribeWorkerPort)),
            NoopWhisperContextPort,
            Arc::new(Mutex::new(ModelOrchestrator::new(
                InjectableMockStore::injected(model_path.clone()),
                MockSequenceDownloader {
                    progress_series: vec![],
                },
            ))),
            std::time::Duration::from_millis(500),
        )));

    transcribe_orch
        .lock()
        .expect("lock")
        .ensure_model()
        .expect("ensure model");

    let hook = TranscribeLifecycleHook::new(
        transcribe_orch.clone(),
        Arc::new(RecordingTranscribeEventEmitter::default()),
    );
    hook.on_capture_phase_changed(PresentationCapturePhase::Capturing);
    assert_eq!(
        transcribe_orch.lock().expect("lock").phase(),
        TranscribePhase::Transcribing
    );

    let result = save_transcript_session_impl(
        &service,
        SaveTranscriptSessionRequest {
            session_id: "session-active-transcribe".to_string(),
            handwriting_markdown: "# meeting".to_string(),
            ai_transcription_markdown: "partial transcript".to_string(),
            ai_transcription_jsonl: None,
        },
    );

    assert!(
        result.success,
        "save must succeed while transcribe is active"
    );
    assert!(result.error.is_none());
    assert!(result.output_directory.is_some());

    let _ = std::fs::remove_file(model_path);
}
