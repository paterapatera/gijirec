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
        Self {
            phase: CapturePhase::Idle,
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
    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert!(emitter.errors().is_empty());
    assert!(emitter.phases().is_empty());
}

#[test]
fn setup_resolves_selection_via_get_selection_and_calls_start_with_selection() {
    let platform = FixedPlatformSupport { supported: true };
    let notifier = RecordingUnsupportedPlatformNotifier::new();
    let emitter = RecordingEventEmitter::new();
    let selection = StubSelectionService::default_unmodified();
    let mut orch = TrackingOrchestrator::new();
    let start_called = Arc::clone(&orch.start_called);
    let calls = Arc::clone(&orch.start_with_selection_calls);

    on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier).expect("setup");

    assert!(selection.get_selection_was_called());
    assert!(
        !start_called.load(Ordering::SeqCst),
        "must not call start()"
    );
    let applied = calls.lock().expect("lock");
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0], DeviceSelection::default());
    assert!(applied[0].resolves_microphone_to_os_default());
    assert!(applied[0].resolves_speaker_to_os_default());
    assert_eq!(orch.phase(), CapturePhase::Capturing);
}

#[test]
fn setup_on_supported_platform_starts_capture_and_emits_phases() {
    let (platform, notifier, emitter, selection) = platform_setup_context(true);
    let (mic, system, mut orch) = make_orchestrator();

    on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier).expect("setup");

    assert_eq!(orch.phase(), CapturePhase::Capturing);
    assert!(mic.is_open());
    assert!(system.is_open());
    assert!(!notifier.was_notified());

    let phases = emitter.phases();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].phase, "starting");
    assert_eq!(phases[1].phase, "capturing");
}

#[test]
fn setup_on_unsupported_platform_does_not_start_capture() {
    let (platform, notifier, emitter, selection) = platform_setup_context(false);
    let (mic, system, mut orch) = make_orchestrator();

    on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier).expect("setup");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert!(!mic.is_open());
    assert!(!system.is_open());
    assert!(notifier.was_notified());
    assert!(!selection.get_selection_was_called());
    assert!(emitter.phases().is_empty());
}

#[test]
fn close_requested_stops_capture_and_releases_streams() {
    let emitter = RecordingEventEmitter::new();
    let (mic, system, mut orch) = make_orchestrator();
    orch.start().expect("start");

    on_close_requested(&mut orch, &emitter).expect("close");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert_capture_streams_closed(&mic, &system);

    let phases = emitter.phases();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].phase, "stopping");
    assert_eq!(phases[1].phase, "idle");
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

#[test]
fn setup_failure_emits_error_and_error_phase() {
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
    let err = on_app_setup(&platform, &mut orch, &selection, &emitter, &notifier)
        .expect_err("setup should fail");
    assert!(matches!(
        err,
        LifecycleError::Orchestrator(CaptureError::MicUnavailable)
    ));

    assert_eq!(orch.phase(), CapturePhase::Error);
    assert_eq!(emitter.errors().len(), 1);
    assert!(!emitter.errors()[0].action_ja.is_empty());
    let phases = emitter.phases();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].phase, "starting");
    assert_eq!(phases[1].phase, "error");
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
    hook.on_capture_started();

    assert_eq!(orch.phase(), CapturePhase::Capturing);
    assert!(mic.is_open());
    assert!(system.is_open());
    assert!(hook.is_active());

    hook.on_capture_stopping();
    on_close_requested(&mut orch, &emitter).expect("close");

    assert_eq!(orch.phase(), CapturePhase::Idle);
    assert_capture_streams_closed(&mic, &system);
    assert!(!hook.is_active());

    let phases = emitter.phases();
    assert_eq!(phases.len(), 4);
    assert_eq!(phases[0].phase, "starting");
    assert_eq!(phases[1].phase, "capturing");
    assert_eq!(phases[2].phase, "stopping");
    assert_eq!(phases[3].phase, "idle");
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
    let worker = ExitHookMockWorker {
        stopped: Arc::clone(&worker_stopped),
        stop_timeout: Arc::clone(&stop_timeout),
    };
    let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(
        ExitHookDummyStore,
        ExitHookDummyDownloader,
    )));
    let orch: Arc<Mutex<dyn TranscribeOrchestrator>> =
        Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
            worker,
            NoopWhisperContextPort,
            model_orch,
            Duration::from_secs(5),
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
        Some(Duration::from_secs(5)),
        "Transcribe worker stop must use 5-second timeout bound"
    );
}

#[test]
fn handle_capture_run_event_exit_and_window_close_invokes_hook_and_stops_worker() {
    let (transcribe_hook, orch, worker_stopped, stop_timeout) = transcribing_exit_hook_fixture();
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);

    let (_, _, capture_orch) = make_orchestrator();
    let state = CaptureLifecycleState::new(
        Arc::new(Mutex::new(capture_orch)),
        Arc::new(StubSelectionService::default_unmodified()),
        Arc::new(FixedPlatformSupport { supported: true }),
        Arc::new(RecordingUnsupportedPlatformNotifier::new()),
    );

    perform_app_exit_shutdown(Some(&transcribe_hook), &state);

    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
    assert!(*worker_stopped.lock().unwrap());
    assert_eq!(*stop_timeout.lock().unwrap(), Some(Duration::from_secs(5)));
}
