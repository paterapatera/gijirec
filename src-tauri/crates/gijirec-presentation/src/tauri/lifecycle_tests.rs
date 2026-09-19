use super::*;
use crate::application::transcribe::model_orchestrator::ModelOrchestrator;
use crate::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use crate::application::transcribe::ports::{
    ModelDownloadProgress, ModelDownloaderPort, ModelStorePort, TranscribeWorkerPort,
};
use crate::tauri::events::RecordingEventEmitter;
use crate::transcribe::test_support::NoopWhisperContextPort;
use crate::transcribe::{TranscribeEmitError, TranscribeEventEmitter};
use gijirec_application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
};
use gijirec_application::device_selection::{DeviceSelectionError, DeviceSelectionService};
use gijirec_domain::audio::DeviceSelection;
use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::transcribe::TranscribeLifecycleHook;
use crate::transcribe::lifecycle_hook::{
    APP_EXIT_TRANSCRIBE_JOIN_TIMEOUT, DEFAULT_TRANSCRIBE_STOP_TIMEOUT,
};

struct StubSelectionService {
    selection: DeviceSelection,
    get_selection_called: Arc<AtomicBool>,
}

impl StubSelectionService {
    fn with_selection(selection: DeviceSelection) -> Self {
        Self {
            selection,
            get_selection_called: Arc::new(AtomicBool::new(false)),
        }
    }

    fn default_unmodified() -> Self {
        Self::with_selection(DeviceSelection::default())
    }

    fn get_selection_was_called(&self) -> bool {
        self.get_selection_called.load(Ordering::SeqCst)
    }
}

impl DeviceSelectionService for StubSelectionService {
    fn list_devices(&self) -> Result<gijirec_domain::audio::AudioDeviceList, DeviceSelectionError> {
        Ok(gijirec_domain::audio::AudioDeviceList::default())
    }

    fn get_selection(&self) -> DeviceSelection {
        self.get_selection_called.store(true, Ordering::SeqCst);
        self.selection.clone()
    }

    fn set_selection(
        &self,
        _selection: DeviceSelection,
    ) -> Result<DeviceSelection, DeviceSelectionError> {
        Ok(self.selection.clone())
    }

    fn set_ui_visible(&self, _visible: bool) {}
}

struct TrackingOrchestrator {
    phase: CapturePhase,
    start_called: Arc<AtomicBool>,
    start_with_selection_calls: Arc<Mutex<Vec<DeviceSelection>>>,
}

impl TrackingOrchestrator {
    fn new() -> Self {
        Self::with_capture_phase(CapturePhase::Idle)
    }

    fn with_capture_phase(phase: CapturePhase) -> Self {
        Self {
            phase,
            start_called: Arc::new(AtomicBool::new(false)),
            start_with_selection_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CaptureOrchestrator for TrackingOrchestrator {
    fn start(&mut self) -> Result<(), CaptureError> {
        self.start_called.store(true, Ordering::SeqCst);
        Err(CaptureError::Internal {
            detail: "start() must not be called from on_app_setup".to_string(),
        })
    }

    fn start_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        if self.phase == CapturePhase::Starting {
            return Err(CaptureError::Internal {
                detail: "capture start blocked in starting-phase shutdown test".to_string(),
            });
        }
        self.start_with_selection_calls
            .lock()
            .expect("lock")
            .push(selection.clone());
        self.phase = CapturePhase::Capturing;
        Ok(())
    }

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.start_with_selection(selection)
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.phase = CapturePhase::Idle;
        Ok(())
    }

    fn phase(&self) -> CapturePhase {
        self.phase
    }

    fn on_device_disconnected(&mut self) -> Result<(), CaptureError> {
        if self.phase != CapturePhase::Capturing {
            return Ok(());
        }
        self.phase = CapturePhase::Error;
        Err(CaptureError::DeviceDisconnected)
    }
}

struct FixedPlatformSupport {
    supported: bool,
}

impl CapturePlatformSupport for FixedPlatformSupport {
    fn is_capture_supported(&self) -> bool {
        self.supported
    }
}

macro_rules! define_lifecycle_mock_capture_port {
    ($mock:ident, $state:ident, $trait:path, $fail_err:expr) => {
        struct $mock {
            state: Arc<Mutex<$state>>,
        }

        struct $state {
            open_ok: bool,
            opened: bool,
        }

        impl $mock {
            fn succeeds() -> Self {
                Self {
                    state: Arc::new(Mutex::new($state {
                        open_ok: true,
                        opened: false,
                    })),
                }
            }

            fn is_open(&self) -> bool {
                self.state.lock().expect("lock").opened
            }
        }

        impl $trait for $mock {
            fn open(&mut self) -> Result<(), CaptureError> {
                let mut s = self.state.lock().expect("lock");
                if s.open_ok {
                    s.opened = true;
                    Ok(())
                } else {
                    Err($fail_err)
                }
            }

            fn close(&mut self) {
                self.state.lock().expect("lock").opened = false;
            }

            fn is_open(&self) -> bool {
                self.state.lock().expect("lock").opened
            }
        }
    };
}

define_lifecycle_mock_capture_port!(
    MockMic,
    MockMicState,
    MicCapturePort,
    CaptureError::MicUnavailable
);
define_lifecycle_mock_capture_port!(
    MockSystem,
    MockSystemState,
    SystemAudioCapturePort,
    CaptureError::SystemAudioUnavailable
);

fn platform_setup_context(
    supported: bool,
) -> (
    FixedPlatformSupport,
    RecordingUnsupportedPlatformNotifier,
    RecordingEventEmitter,
    StubSelectionService,
) {
    (
        FixedPlatformSupport { supported },
        RecordingUnsupportedPlatformNotifier::new(),
        RecordingEventEmitter::new(),
        StubSelectionService::default_unmodified(),
    )
}

fn assert_capture_streams_closed(mic: &MockMic, system: &MockSystem) {
    assert!(!mic.is_open());
    assert!(!system.is_open());
}

fn assert_disconnected_noop_idle(orch: &impl CaptureOrchestrator, emitter: &RecordingEventEmitter) {
    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert!(emitter.errors().is_empty());
    assert!(emitter.phases().is_empty());
}

struct SetupAssertionContext<'a> {
    mic: &'a MockMic,
    system: &'a MockSystem,
    orch: &'a dyn CaptureOrchestrator,
    notifier: &'a RecordingUnsupportedPlatformNotifier,
    emitter: &'a RecordingEventEmitter,
    selection: &'a StubSelectionService,
    expect_notifier: bool,
}

impl SetupAssertionContext<'_> {
    fn assert_streams_closed(&self) {
        assert_eq!(self.orch.phase(), CapturePhase::Idle);
        assert!(!self.mic.is_open());
        assert!(!self.system.is_open());
        assert_eq!(self.notifier.was_notified(), self.expect_notifier);
        assert!(!self.selection.get_selection_was_called());
        assert!(self.emitter.phases().is_empty());
    }
}

fn assert_close_stopping_then_idle(
    emitter: &RecordingEventEmitter,
    mic: &MockMic,
    system: &MockSystem,
) {
    assert_capture_streams_closed(mic, system);
    let phases = emitter.phases();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].phase, "stopping");
    assert_eq!(phases[1].phase, "idle");
}

fn make_orchestrator() -> (
    MockMic,
    MockSystem,
    DefaultCaptureOrchestrator<MockMic, MockSystem>,
) {
    let mic = MockMic::succeeds();
    let system = MockSystem::succeeds();
    let orch = DefaultCaptureOrchestrator::new(
        MockMic {
            state: mic.state.clone(),
        },
        MockSystem {
            state: system.state.clone(),
        },
    );
    (mic, system, orch)
}

#[test]
fn handle_capture_device_disconnected_emits_device_disconnected_and_error_phase() {
    let (mic, system, mut orch) = make_orchestrator();
    orch.start_with_selection(&DeviceSelection::default())
        .expect("start capturing");
    assert_eq!(orch.phase(), CapturePhase::Capturing);
    assert!(mic.is_open());
    assert!(system.is_open());

    let emitter = RecordingEventEmitter::new();
    handle_capture_device_disconnected(&mut orch, &emitter, None).expect("disconnect");

    assert_eq!(orch.phase(), CapturePhase::Error);
    assert!(!mic.is_open());
    assert!(!system.is_open());

    let errors = emitter.errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "DEVICE_DISCONNECTED");
    assert!(!errors[0].action_ja.is_empty());

    let phases = emitter.phases();
    assert!(
        phases.iter().any(|payload| payload.phase == "error"),
        "expected error phase event: {phases:?}"
    );
}

#[test]
fn handle_capture_device_disconnected_noop_when_not_capturing() {
    let (_, _, mut orch) = make_orchestrator();
    let emitter = RecordingEventEmitter::new();
    handle_capture_device_disconnected(&mut orch, &emitter, None).expect("noop");
    assert_disconnected_noop_idle(&orch, &emitter);
}

#[test]
fn setup_on_supported_platform_leaves_idle_without_start_with_selection() {
    let platform = FixedPlatformSupport { supported: true };
    let notifier = RecordingUnsupportedPlatformNotifier::new();
    let emitter = RecordingEventEmitter::new();
    let selection = StubSelectionService::default_unmodified();
    let mut orch = TrackingOrchestrator::new();
    let start_called = Arc::clone(&orch.start_called);
    let calls = Arc::clone(&orch.start_with_selection_calls);

    on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier).expect("setup");

    assert!(
        !selection.get_selection_was_called(),
        "setup must not resolve device selection before session start"
    );
    assert!(
        !start_called.load(Ordering::SeqCst),
        "must not call start()"
    );
    let applied = calls.lock().expect("lock");
    assert!(
        applied.is_empty(),
        "start_with_selection must not run on app setup"
    );
    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert!(emitter.phases().is_empty());
    assert!(!notifier.was_notified());
}

fn run_setup_platform_assertion(supported: bool) {
    let (platform, notifier, emitter, selection) = platform_setup_context(supported);
    let (mic, system, mut orch) = make_orchestrator();

    on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier).expect("setup");

    SetupAssertionContext {
        mic: &mic,
        system: &system,
        orch: &orch,
        notifier: &notifier,
        emitter: &emitter,
        selection: &selection,
        expect_notifier: !supported,
    }
    .assert_streams_closed();
}

#[test]
fn setup_on_supported_platform_does_not_open_capture_streams() {
    run_setup_platform_assertion(true);
}

#[test]
fn setup_on_unsupported_platform_does_not_start_capture() {
    run_setup_platform_assertion(false);
}

#[test]
fn close_requested_stops_capture_and_releases_streams() {
    let emitter = RecordingEventEmitter::new();
    let (mic, system, mut orch) = make_orchestrator();
    orch.start().expect("start");

    on_close_requested(&mut orch, &emitter).expect("close");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert_close_stopping_then_idle(&emitter, &mic, &system);
}

#[test]
fn app_exit_stops_capture_like_close_requested() {
    let emitter = RecordingEventEmitter::new();
    let (mic, system, mut orch) = make_orchestrator();
    orch.start().expect("start");

    on_app_exit(&mut orch, &emitter).expect("exit");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert_capture_streams_closed(&mic, &system);
}

#[test]
fn shutdown_from_idle_is_idempotent() {
    let emitter = RecordingEventEmitter::new();
    let (_, _, mut orch) = make_orchestrator();

    on_close_requested(&mut orch, &emitter).expect("close idle");
    on_app_exit(&mut orch, &emitter).expect("exit idle");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert!(emitter.phases().is_empty());
}

fn assert_shutdown_stops_starting_capture(
    run: impl FnOnce(
        &mut dyn CaptureOrchestrator,
        &dyn crate::tauri::events::CaptureEventEmitter,
    ) -> Result<(), LifecycleError>,
) {
    let emitter = RecordingEventEmitter::new();
    let mut orch = TrackingOrchestrator::with_capture_phase(CapturePhase::Starting);
    run(&mut orch, &emitter).expect("shutdown during starting");
    assert_eq!(orch.phase(), CapturePhase::Idle);
    let phases = emitter.phases();
    assert!(
        phases.iter().any(|p| p.phase == "stopping"),
        "shutdown during starting must emit stopping"
    );
    assert!(
        phases.last().is_some_and(|p| p.phase == "idle"),
        "shutdown during starting must end at idle"
    );
}

/// Task 11.2 / req 6.4: window close while capture is still `starting` stops cleanly.
#[test]
fn integration_task_11_2_close_requested_while_capture_phase_starting() {
    assert_shutdown_stops_starting_capture(on_close_requested);
}

/// Task 11.2 / req 6.4: app exit while capture is `starting` stops without leaking streams.
#[test]
fn integration_task_11_2_app_exit_while_capture_phase_starting() {
    assert_shutdown_stops_starting_capture(on_app_exit);
}

fn run_transcribing_window_exit_and_assert_capture_idle(
    state: &CaptureLifecycleState,
    idle_message: &str,
) {
    let recording = Arc::new(RecordingEventEmitter::new());
    state.init_emitter(recording.clone());
    run_transcribing_app_exit_shutdown(state, recording.as_ref());
    let capture = state.orchestrator.lock().expect("lock");
    assert_eq!(capture.phase(), CapturePhase::Idle, "{idle_message}");
}

/// Task 11.2 / req 6.4: main-window exit path stops transcribe while capture is starting.
#[test]
fn integration_task_11_2_window_exit_stops_transcribe_during_capture_starting() {
    let state = lifecycle_state_with_orchestrator(Arc::new(Mutex::new(
        TrackingOrchestrator::with_capture_phase(CapturePhase::Starting),
    )));
    run_transcribing_window_exit_and_assert_capture_idle(
        &state,
        "exit must stop capture even from starting phase",
    );
}

#[test]
fn setup_on_supported_platform_stays_idle_when_devices_would_fail_if_started() {
    let platform = FixedPlatformSupport { supported: true };
    let notifier = RecordingUnsupportedPlatformNotifier::new();
    let emitter = RecordingEventEmitter::new();

    let mic = MockMic {
        state: Arc::new(Mutex::new(MockMicState {
            open_ok: false,
            opened: false,
        })),
    };
    let system = MockSystem::succeeds();
    let mut orch = DefaultCaptureOrchestrator::new(mic, system);

    let selection = StubSelectionService::default_unmodified();
    on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier).expect("setup");

    assert_disconnected_noop_idle(&orch, &emitter);
}

#[test]
fn unsupported_message_mentions_linux() {
    assert!(UNSUPPORTED_PLATFORM_MESSAGE_JA.contains("Linux"));
}

struct RecordingProcessingHook {
    active: Arc<Mutex<bool>>,
}

impl RecordingProcessingHook {
    fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(false)),
        }
    }

    fn is_active(&self) -> bool {
        *self.active.lock().expect("lock")
    }
}

impl CaptureProcessingHook for RecordingProcessingHook {
    fn on_capture_started(&self) {
        *self.active.lock().expect("lock") = true;
    }

    fn on_capture_stopping(&self) {
        *self.active.lock().expect("lock") = false;
    }
}

// Integration Test 3 (lifecycle): start → capturing → stop → idle でストリーム解放
#[test]
fn integration_lifecycle_start_stop_releases_streams_and_processing_hook() {
    let emitter = RecordingEventEmitter::new();
    let hook = Arc::new(RecordingProcessingHook::new());
    let (mic, system, mut orch) = make_orchestrator();

    let selection = StubSelectionService::default_unmodified();
    on_app_setup(
        &FixedPlatformSupport { supported: true },
        &mut orch,
        &selection,
        &emitter,
        &RecordingUnsupportedPlatformNotifier::new(),
    )
    .expect("setup");
    assert_eq!(orch.phase(), CapturePhase::Idle);
    orch.start_with_selection(&selection.get_selection())
        .expect("session start");
    hook.on_capture_started();

    assert_eq!(orch.phase(), CapturePhase::Capturing);
    assert!(mic.is_open());
    assert!(system.is_open());
    assert!(hook.is_active());

    hook.on_capture_stopping();
    on_close_requested(&mut orch, &emitter).expect("close");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert!(!hook.is_active());
    assert_close_stopping_then_idle(&emitter, &mic, &system);
}

struct ExitHookMockWorker {
    stopped: Arc<Mutex<bool>>,
    stop_timeout: Arc<Mutex<Option<Duration>>>,
}

macro_rules! noop_transcribe_worker_prelude {
    () => {
        fn prepare_model_path(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
            Ok(())
        }

        fn spawn(&mut self) -> Result<(), TranscribeError> {
            Ok(())
        }
    };
}

impl TranscribeWorkerPort for ExitHookMockWorker {
    noop_transcribe_worker_prelude!();

    fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError> {
        *self.stopped.lock().unwrap() = true;
        *self.stop_timeout.lock().unwrap() = Some(timeout);
        Ok(())
    }
}

struct ExitHookDummyStore;

impl ModelStorePort for ExitHookDummyStore {
    fn model_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/model")
    }

    fn model_path_for(
        &self,
        _variant: gijirec_domain::transcribe::WhisperModelVariant,
    ) -> std::path::PathBuf {
        self.model_path()
    }

    fn verify(&self, _expected: Option<&str>) -> Result<std::path::PathBuf, TranscribeError> {
        Ok(std::path::PathBuf::from("/tmp/model"))
    }

    fn verify_variant(
        &self,
        _variant: gijirec_domain::transcribe::WhisperModelVariant,
        expected: Option<&str>,
    ) -> Result<std::path::PathBuf, TranscribeError> {
        self.verify(expected)
    }

    fn file_exists(&self, _variant: gijirec_domain::transcribe::WhisperModelVariant) -> bool {
        true
    }
}

struct ExitHookDummyDownloader;

impl ModelDownloaderPort for ExitHookDummyDownloader {
    fn download(
        &self,
        _url: &str,
        _destination: &std::path::Path,
        _on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        Ok(())
    }
}

struct ExitHookDummyEmitter;

impl TranscribeEventEmitter for ExitHookDummyEmitter {
    fn emit_phase_changed(&self, _phase: TranscribePhase) -> Result<(), TranscribeEmitError> {
        Ok(())
    }

    fn emit_model_progress(
        &self,
        _progress: &ModelDownloadProgress,
    ) -> Result<(), TranscribeEmitError> {
        Ok(())
    }

    fn emit_error(&self, _error: &TranscribeError) -> Result<(), TranscribeEmitError> {
        Ok(())
    }
}

#[allow(clippy::type_complexity)]
fn transcribing_exit_hook_fixture() -> (
    Arc<TranscribeLifecycleHook>,
    Arc<Mutex<dyn TranscribeOrchestrator>>,
    Arc<Mutex<bool>>,
    Arc<Mutex<Option<Duration>>>,
) {
    let worker_stopped = Arc::new(Mutex::new(false));
    let stop_timeout = Arc::new(Mutex::new(None));
    let worker = Arc::new(Mutex::new(ExitHookMockWorker {
        stopped: Arc::clone(&worker_stopped),
        stop_timeout: Arc::clone(&stop_timeout),
    }));
    let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(
        ExitHookDummyStore,
        ExitHookDummyDownloader,
    )));
    let orch: Arc<Mutex<dyn TranscribeOrchestrator>> =
        Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
            Arc::clone(&worker),
            NoopWhisperContextPort,
            model_orch,
            DEFAULT_TRANSCRIBE_STOP_TIMEOUT,
        )));
    orch.lock().unwrap().ensure_model().expect("ensure model");
    let hook = Arc::new(TranscribeLifecycleHook::new(
        orch.clone(),
        Arc::new(ExitHookDummyEmitter),
    ));
    hook.on_capture_phase_changed(CapturePhase::Capturing);
    (hook, orch, worker_stopped, stop_timeout)
}

#[test]
fn window_close_and_app_exit_calls_transcribe_hook_and_stops_worker() {
    let (transcribe_hook, orch, worker_stopped, stop_timeout) = transcribing_exit_hook_fixture();
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);

    transcribe_hook.on_app_exit();
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
    assert!(
        *worker_stopped.lock().unwrap(),
        "Transcribe worker must be stopped and joined"
    );
    assert_eq!(
        *stop_timeout.lock().unwrap(),
        Some(APP_EXIT_TRANSCRIBE_JOIN_TIMEOUT),
        "app exit must use a short worker join bound"
    );
}

fn lifecycle_state_with_orchestrator(
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
) -> CaptureLifecycleState {
    CaptureLifecycleState::new(
        orchestrator,
        Arc::new(StubSelectionService::default_unmodified()),
        Arc::new(FixedPlatformSupport { supported: true }),
        Arc::new(RecordingUnsupportedPlatformNotifier::new()),
    )
}

fn stub_capture_lifecycle_state(
    capture_orch: DefaultCaptureOrchestrator<MockMic, MockSystem>,
) -> CaptureLifecycleState {
    lifecycle_state_with_orchestrator(Arc::new(Mutex::new(capture_orch)))
}

fn assert_transcribe_worker_stopped_on_exit(
    orch: &Arc<Mutex<dyn TranscribeOrchestrator>>,
    worker_stopped: &Arc<Mutex<bool>>,
    stop_timeout: &Arc<Mutex<Option<Duration>>>,
) {
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
    assert!(*worker_stopped.lock().unwrap());
    assert_eq!(
        *stop_timeout.lock().unwrap(),
        Some(APP_EXIT_TRANSCRIBE_JOIN_TIMEOUT)
    );
}

fn assert_exit_shutdown_emits_capture_stopping_then_idle(recording: &RecordingEventEmitter) {
    let phases = recording.phases();
    assert!(phases.iter().any(|p| p.phase == "stopping"));
    assert!(phases.last().is_some_and(|p| p.phase == "idle"));
}

fn run_transcribing_app_exit_shutdown(
    state: &CaptureLifecycleState,
    recording: &RecordingEventEmitter,
) {
    let (transcribe_hook, transcribe_orch, worker_stopped, stop_timeout) =
        transcribing_exit_hook_fixture();
    assert_eq!(
        transcribe_orch.lock().unwrap().phase(),
        TranscribePhase::Transcribing
    );
    perform_app_exit_shutdown(Some(&transcribe_hook), state);
    assert_transcribe_worker_stopped_on_exit(&transcribe_orch, &worker_stopped, &stop_timeout);
    assert_exit_shutdown_emits_capture_stopping_then_idle(recording);
}

#[test]
fn handle_capture_run_event_exit_and_window_close_invokes_hook_and_stops_worker() {
    let (transcribe_hook, orch, worker_stopped, stop_timeout) = transcribing_exit_hook_fixture();
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);

    let (_, _, capture_orch) = make_orchestrator();
    let state = stub_capture_lifecycle_state(capture_orch);

    perform_app_exit_shutdown(Some(&transcribe_hook), &state);

    assert_transcribe_worker_stopped_on_exit(&orch, &worker_stopped, &stop_timeout);
}

/// Task 12.3 / req 5.6: exit shutdown stops active capture streams and transcribe worker (lifecycle regression).
#[test]
fn integration_task_12_3_window_exit_stops_capture_and_transcribe() {
    let (mic, system, mut capture_orch) = make_orchestrator();
    capture_orch
        .start_with_selection(&DeviceSelection::default())
        .expect("capture start");
    assert_eq!(capture_orch.phase(), CapturePhase::Capturing);
    assert!(mic.is_open());
    assert!(system.is_open());

    let state = stub_capture_lifecycle_state(capture_orch);
    run_transcribing_window_exit_and_assert_capture_idle(
        &state,
        "capture orchestrator must be idle after exit shutdown",
    );
    assert_capture_streams_closed(&mic, &system);
}
