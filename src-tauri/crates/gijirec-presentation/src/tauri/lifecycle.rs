//! Tauri lifecycle hooks: sync app startup/shutdown with capture orchestration.

use crate::tauri::events::CaptureEventEmitter;
use crate::tauri::observability;
use crate::transcribe::TranscribeLifecycleHook;
use gijirec_application::capture::orchestrator::CaptureOrchestrator;
use gijirec_domain::audio::{CaptureError, CapturePhase};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Builder, Manager, RunEvent, Runtime, WindowEvent};

/// Japanese message shown when capture is unsupported (Linux).
pub const UNSUPPORTED_PLATFORM_MESSAGE_JA: &str = "gijirec Audio Capture は Linux をサポートしていません。Windows または macOS でご利用ください。";

/// Returns whether the current OS supports audio capture.
pub fn is_capture_supported_os() -> bool {
    !cfg!(target_os = "linux")
}

/// Platform support probe (injectable for tests).
pub trait CapturePlatformSupport: Send + Sync {
    fn is_capture_supported(&self) -> bool;
}

/// Uses compile-time OS detection.
pub struct OsCapturePlatformSupport;

impl CapturePlatformSupport for OsCapturePlatformSupport {
    fn is_capture_supported(&self) -> bool {
        is_capture_supported_os()
    }
}

/// Notifies the user that capture is unavailable on this platform.
pub trait UnsupportedPlatformNotifier: Send + Sync {
    fn notify_unsupported(&self);
}

/// Errors from lifecycle handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    Orchestrator(CaptureError),
    Emitter(String),
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Orchestrator(err) => write!(f, "orchestrator error: {err}"),
            Self::Emitter(msg) => write!(f, "emitter error: {msg}"),
        }
    }
}

impl std::error::Error for LifecycleError {}

/// Hook for starting/stopping the capture processing thread (task 7.2).
pub trait CaptureProcessingHook: Send + Sync {
    fn on_capture_started(&self);
    fn on_capture_stopping(&self);
}

/// Starts capture on app setup when the platform is supported.
pub fn on_app_setup(
    platform: &dyn CapturePlatformSupport,
    orchestrator: &mut dyn CaptureOrchestrator,
    emitter: &dyn CaptureEventEmitter,
    notifier: &dyn UnsupportedPlatformNotifier,
) -> Result<(), LifecycleError> {
    if !platform.is_capture_supported() {
        notifier.notify_unsupported();
        return Ok(());
    }

    if orchestrator.phase() == CapturePhase::Idle {
        emit_phase(emitter, CapturePhase::Starting)?;
    }

    match orchestrator.start() {
        Ok(()) => emit_phase(emitter, orchestrator.phase())?,
        Err(err) => {
            emit_error(emitter, err.clone())?;
            emit_phase(emitter, CapturePhase::Error)?;
            return Err(LifecycleError::Orchestrator(err));
        }
    }

    Ok(())
}

/// Stops capture when the user closes the main window.
pub fn on_close_requested(
    orchestrator: &mut dyn CaptureOrchestrator,
    emitter: &dyn CaptureEventEmitter,
) -> Result<(), LifecycleError> {
    on_app_shutdown(orchestrator, emitter)
}

/// Stops capture on application exit (OS quit, etc.).
pub fn on_app_exit(
    orchestrator: &mut dyn CaptureOrchestrator,
    emitter: &dyn CaptureEventEmitter,
) -> Result<(), LifecycleError> {
    on_app_shutdown(orchestrator, emitter)
}

fn on_app_shutdown(
    orchestrator: &mut dyn CaptureOrchestrator,
    emitter: &dyn CaptureEventEmitter,
) -> Result<(), LifecycleError> {
    if orchestrator.phase() == CapturePhase::Idle {
        return Ok(());
    }

    emit_phase(emitter, CapturePhase::Stopping)?;

    orchestrator.stop().map_err(LifecycleError::Orchestrator)?;

    emit_phase(emitter, CapturePhase::Idle)?;
    Ok(())
}

fn emit_phase(
    emitter: &dyn CaptureEventEmitter,
    phase: CapturePhase,
) -> Result<(), LifecycleError> {
    observability::log_phase_transition(phase);
    emitter
        .emit_phase_changed(phase)
        .map_err(|e| LifecycleError::Emitter(e.to_string()))
}

fn emit_error(
    emitter: &dyn CaptureEventEmitter,
    error: CaptureError,
) -> Result<(), LifecycleError> {
    emitter
        .emit_error(error)
        .map_err(|e| LifecycleError::Emitter(e.to_string()))
}

/// Records unsupported-platform notifications in tests.
#[derive(Debug, Default)]
pub struct RecordingUnsupportedPlatformNotifier {
    notified: std::sync::Mutex<bool>,
}

impl RecordingUnsupportedPlatformNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn was_notified(&self) -> bool {
        *self.notified.lock().expect("lock")
    }
}

impl UnsupportedPlatformNotifier for RecordingUnsupportedPlatformNotifier {
    fn notify_unsupported(&self) {
        *self.notified.lock().expect("lock") = true;
    }
}

/// Production notifier: blocking dialog on Linux, no-op elsewhere.
pub struct OsUnsupportedPlatformNotifier;

impl UnsupportedPlatformNotifier for OsUnsupportedPlatformNotifier {
    fn notify_unsupported(&self) {
        #[cfg(target_os = "linux")]
        {
            rfd::MessageDialog::new()
                .set_title("gijirec Audio Capture")
                .set_description(UNSUPPORTED_PLATFORM_MESSAGE_JA)
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        }
    }
}

/// Managed state for Tauri lifecycle wiring (consumed by task 7.1).
pub struct CaptureLifecycleState {
    orchestrator: Mutex<Box<dyn CaptureOrchestrator>>,
    emitter: Mutex<Option<Arc<dyn CaptureEventEmitter>>>,
    platform: Arc<dyn CapturePlatformSupport>,
    notifier: Arc<dyn UnsupportedPlatformNotifier>,
    processing: Mutex<Option<Arc<dyn CaptureProcessingHook>>>,
}

impl CaptureLifecycleState {
    pub fn new(
        orchestrator: Box<dyn CaptureOrchestrator>,
        platform: Arc<dyn CapturePlatformSupport>,
        notifier: Arc<dyn UnsupportedPlatformNotifier>,
    ) -> Self {
        Self {
            orchestrator: Mutex::new(orchestrator),
            emitter: Mutex::new(None),
            platform,
            notifier,
            processing: Mutex::new(None),
        }
    }

    pub fn set_processing_hook(&self, hook: Arc<dyn CaptureProcessingHook>) {
        *self.processing.lock().expect("lock") = Some(hook);
    }

    pub fn add_processing_hook(&self, hook: Arc<dyn CaptureProcessingHook>) {
        let mut guard = self.processing.lock().expect("lock");
        if let Some(existing) = guard.take() {
            *guard = Some(Arc::new(ChainedProcessingHook {
                first: existing,
                second: hook,
            }));
        } else {
            *guard = Some(hook);
        }
    }

    pub fn init_emitter(&self, emitter: Arc<dyn CaptureEventEmitter>) {
        *self.emitter.lock().expect("lock") = Some(emitter);
    }

    /// Current capture phase for frontend mount sync (missed lifecycle events).
    pub fn current_phase_payload(
        &self,
    ) -> Result<crate::tauri::events::CapturePhaseChangedPayload, String> {
        let orchestrator = self
            .orchestrator
            .lock()
            .map_err(|_| "capture orchestrator lock poisoned".to_string())?;
        Ok(crate::tauri::events::build_phase_payload(
            orchestrator.phase(),
        ))
    }

    fn emitter(&self) -> Option<Arc<dyn CaptureEventEmitter>> {
        self.emitter.lock().expect("lock").clone()
    }
}

struct ChainedProcessingHook {
    first: Arc<dyn CaptureProcessingHook>,
    second: Arc<dyn CaptureProcessingHook>,
}

impl CaptureProcessingHook for ChainedProcessingHook {
    fn on_capture_started(&self) {
        self.first.on_capture_started();
        self.second.on_capture_started();
    }

    fn on_capture_stopping(&self) {
        self.first.on_capture_stopping();
        self.second.on_capture_stopping();
    }
}

pub(crate) fn perform_app_exit_shutdown(
    transcribe_hook: Option<&TranscribeLifecycleHook>,
    capture_state: &CaptureLifecycleState,
) {
    if let Some(hook) = transcribe_hook {
        hook.on_app_exit();
    }
    if let Some(processing) = capture_state.processing.lock().expect("lock").as_ref() {
        processing.on_capture_stopping();
    }
    if let Some(emitter) = capture_state.emitter() {
        let mut orch = capture_state.orchestrator.lock().expect("lock");
        let _ = on_app_shutdown(orch.as_mut(), emitter.as_ref());
    }
}

pub(crate) fn handle_window_close_requested<R: Runtime>(app: &AppHandle<R>) {
    let transcribe_hook = app.try_state::<Arc<TranscribeLifecycleHook>>();
    let state = app.state::<CaptureLifecycleState>();
    perform_app_exit_shutdown(transcribe_hook.as_ref().map(|h| &***h), &state);
}

/// Initializes capture emitter and starts capture on app setup.
///
/// Tauri allows only one `.setup()` callback; call this from the composition root's
/// unified setup instead of registering a second handler.
pub fn run_capture_app_setup<R: Runtime>(app: &AppHandle<R>) -> Result<(), LifecycleError> {
    let managed = app.state::<CaptureLifecycleState>();
    managed.init_emitter(Arc::new(
        crate::tauri::events::TauriCaptureEventEmitter::new(app.clone()),
    ));
    let emitter = managed
        .emitter()
        .expect("emitter must be initialized in setup");
    let mut orch = managed.orchestrator.lock().expect("lock");
    on_app_setup(
        managed.platform.as_ref(),
        orch.as_mut(),
        emitter.as_ref(),
        managed.notifier.as_ref(),
    )?;

    if orch.phase() == CapturePhase::Capturing
        && let Some(processing) = managed.processing.lock().expect("lock").as_ref()
    {
        processing.on_capture_started();
    }
    Ok(())
}

/// Registers managed state, window close, and documents exit handling for capture lifecycle.
pub fn attach_capture_lifecycle<R: Runtime>(
    builder: Builder<R>,
    state: CaptureLifecycleState,
) -> Builder<R> {
    builder.manage(state).on_window_event(|window, event| {
        if matches!(event, WindowEvent::CloseRequested { .. }) {
            handle_window_close_requested(window.app_handle());
        }
    })
}

/// Call from `app.run` to stop capture on `RunEvent::Exit` (task 7.1).
pub fn handle_capture_run_event<R: Runtime>(app: &AppHandle<R>, event: &RunEvent) {
    if matches!(event, RunEvent::Exit) {
        let transcribe_hook = app.try_state::<Arc<TranscribeLifecycleHook>>();
        let state = app.state::<CaptureLifecycleState>();
        perform_app_exit_shutdown(transcribe_hook.as_ref().map(|h| &***h), &state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::transcribe::model_orchestrator::{
        ModelOrchestrator, ModelOrchestratorConfig,
    };
    use crate::application::transcribe::orchestrator::{
        DefaultTranscribeOrchestrator, TranscribeOrchestrator,
    };
    use crate::application::transcribe::ports::{
        ModelDownloadProgress, ModelDownloaderPort, ModelStorePort, TranscribeWorkerPort,
        WhisperContextPort,
    };
    use crate::tauri::events::RecordingEventEmitter;
    use crate::transcribe::{TranscribeEmitError, TranscribeEventEmitter};
    use gijirec_application::capture::orchestrator::{
        CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
    };
    use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct FixedPlatformSupport {
        supported: bool,
    }

    impl CapturePlatformSupport for FixedPlatformSupport {
        fn is_capture_supported(&self) -> bool {
            self.supported
        }
    }

    struct MockMic {
        state: Arc<Mutex<MockMicState>>,
    }

    struct MockMicState {
        open_ok: bool,
        opened: bool,
    }

    impl MockMic {
        fn succeeds() -> Self {
            Self {
                state: Arc::new(Mutex::new(MockMicState {
                    open_ok: true,
                    opened: false,
                })),
            }
        }

        fn is_open(&self) -> bool {
            self.state.lock().expect("lock").opened
        }
    }

    impl MicCapturePort for MockMic {
        fn open(&mut self) -> Result<(), CaptureError> {
            let mut s = self.state.lock().expect("lock");
            if s.open_ok {
                s.opened = true;
                Ok(())
            } else {
                Err(CaptureError::MicUnavailable)
            }
        }

        fn close(&mut self) {
            self.state.lock().expect("lock").opened = false;
        }

        fn is_open(&self) -> bool {
            self.state.lock().expect("lock").opened
        }
    }

    struct MockSystem {
        state: Arc<Mutex<MockSystemState>>,
    }

    struct MockSystemState {
        open_ok: bool,
        opened: bool,
    }

    impl MockSystem {
        fn succeeds() -> Self {
            Self {
                state: Arc::new(Mutex::new(MockSystemState {
                    open_ok: true,
                    opened: false,
                })),
            }
        }

        fn is_open(&self) -> bool {
            self.state.lock().expect("lock").opened
        }
    }

    impl SystemAudioCapturePort for MockSystem {
        fn open(&mut self) -> Result<(), CaptureError> {
            let mut s = self.state.lock().expect("lock");
            if s.open_ok {
                s.opened = true;
                Ok(())
            } else {
                Err(CaptureError::SystemAudioUnavailable)
            }
        }

        fn close(&mut self) {
            self.state.lock().expect("lock").opened = false;
        }

        fn is_open(&self) -> bool {
            self.state.lock().expect("lock").opened
        }
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
    fn setup_on_supported_platform_starts_capture_and_emits_phases() {
        let platform = FixedPlatformSupport { supported: true };
        let notifier = RecordingUnsupportedPlatformNotifier::new();
        let emitter = RecordingEventEmitter::new();
        let (mic, system, mut orch) = make_orchestrator();

        on_app_setup(&platform, &mut orch, &emitter, &notifier).expect("setup");

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
        let platform = FixedPlatformSupport { supported: false };
        let notifier = RecordingUnsupportedPlatformNotifier::new();
        let emitter = RecordingEventEmitter::new();
        let (mic, system, mut orch) = make_orchestrator();

        on_app_setup(&platform, &mut orch, &emitter, &notifier).expect("setup");

        assert_eq!(orch.phase(), CapturePhase::Idle);
        assert!(!mic.is_open());
        assert!(!system.is_open());
        assert!(notifier.was_notified());
        assert!(emitter.phases().is_empty());
    }

    #[test]
    fn close_requested_stops_capture_and_releases_streams() {
        let emitter = RecordingEventEmitter::new();
        let (mic, system, mut orch) = make_orchestrator();
        orch.start().expect("start");

        on_close_requested(&mut orch, &emitter).expect("close");

        assert_eq!(orch.phase(), CapturePhase::Idle);
        assert!(!mic.is_open());
        assert!(!system.is_open());

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
        assert!(!mic.is_open());
        assert!(!system.is_open());
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

        let err =
            on_app_setup(&platform, &mut orch, &emitter, &notifier).expect_err("setup should fail");
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

        on_app_setup(
            &FixedPlatformSupport { supported: true },
            &mut orch,
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
        assert!(!mic.is_open());
        assert!(!system.is_open());
        assert!(!hook.is_active());

        let phases = emitter.phases();
        assert_eq!(phases.len(), 4);
        assert_eq!(phases[0].phase, "starting");
        assert_eq!(phases[1].phase, "capturing");
        assert_eq!(phases[2].phase, "stopping");
        assert_eq!(phases[3].phase, "idle");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn window_close_and_app_exit_calls_transcribe_hook_and_stops_worker() {
        struct MockWorker {
            stopped: Arc<Mutex<bool>>,
            stop_timeout: Arc<Mutex<Option<Duration>>>,
        }
        impl TranscribeWorkerPort for MockWorker {
            fn spawn(&mut self) -> Result<(), TranscribeError> {
                Ok(())
            }
            fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError> {
                *self.stopped.lock().unwrap() = true;
                *self.stop_timeout.lock().unwrap() = Some(timeout);
                Ok(())
            }
        }
        struct DummyCtx;
        impl WhisperContextPort for DummyCtx {
            fn load_model(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
                Ok(())
            }
        }
        struct DummyStore;
        impl ModelStorePort for DummyStore {
            fn model_path(&self) -> std::path::PathBuf {
                std::path::PathBuf::from("/tmp/model")
            }
            fn verify(
                &self,
                _expected: Option<&str>,
            ) -> Result<std::path::PathBuf, TranscribeError> {
                Ok(std::path::PathBuf::from("/tmp/model"))
            }
        }
        struct DummyDownloader;
        impl ModelDownloaderPort for DummyDownloader {
            fn download(
                &self,
                _url: &str,
                _destination: &std::path::Path,
                _on_progress: &mut dyn FnMut(ModelDownloadProgress),
            ) -> Result<(), TranscribeError> {
                Ok(())
            }
        }

        struct DummyEmitter;
        impl TranscribeEventEmitter for DummyEmitter {
            fn emit_phase_changed(
                &self,
                _phase: TranscribePhase,
            ) -> Result<(), TranscribeEmitError> {
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

        let worker_stopped = Arc::new(Mutex::new(false));
        let stop_timeout = Arc::new(Mutex::new(None));
        let worker = MockWorker {
            stopped: Arc::clone(&worker_stopped),
            stop_timeout: Arc::clone(&stop_timeout),
        };
        let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(
            DummyStore,
            DummyDownloader,
            ModelOrchestratorConfig {
                model_url: "url".to_string(),
                expected_sha256: "hash".to_string(),
            },
        )));
        let orch = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
            worker,
            DummyCtx,
            model_orch,
            Duration::from_secs(5),
        )));
        orch.lock().unwrap().ensure_model().expect("ensure model");
        let transcribe_hook = Arc::new(TranscribeLifecycleHook::new(
            orch.clone() as Arc<Mutex<dyn TranscribeOrchestrator>>,
            Arc::new(DummyEmitter),
        ));
        transcribe_hook.on_capture_phase_changed(CapturePhase::Capturing);
        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);

        // When app exit / window close triggers hook
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
    #[allow(clippy::too_many_lines)]
    fn handle_capture_run_event_exit_and_window_close_invokes_hook_and_stops_worker() {
        struct MockWorker {
            stopped: Arc<Mutex<bool>>,
            stop_timeout: Arc<Mutex<Option<Duration>>>,
        }
        impl TranscribeWorkerPort for MockWorker {
            fn spawn(&mut self) -> Result<(), TranscribeError> {
                Ok(())
            }
            fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError> {
                *self.stopped.lock().unwrap() = true;
                *self.stop_timeout.lock().unwrap() = Some(timeout);
                Ok(())
            }
        }
        struct DummyCtx;
        impl WhisperContextPort for DummyCtx {
            fn load_model(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
                Ok(())
            }
        }
        struct DummyStore;
        impl ModelStorePort for DummyStore {
            fn model_path(&self) -> std::path::PathBuf {
                std::path::PathBuf::from("/tmp/model")
            }
            fn verify(
                &self,
                _expected: Option<&str>,
            ) -> Result<std::path::PathBuf, TranscribeError> {
                Ok(std::path::PathBuf::from("/tmp/model"))
            }
        }
        struct DummyDownloader;
        impl ModelDownloaderPort for DummyDownloader {
            fn download(
                &self,
                _url: &str,
                _destination: &std::path::Path,
                _on_progress: &mut dyn FnMut(ModelDownloadProgress),
            ) -> Result<(), TranscribeError> {
                Ok(())
            }
        }
        struct DummyEmitter;
        impl TranscribeEventEmitter for DummyEmitter {
            fn emit_phase_changed(
                &self,
                _phase: TranscribePhase,
            ) -> Result<(), TranscribeEmitError> {
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

        let worker_stopped = Arc::new(Mutex::new(false));
        let stop_timeout = Arc::new(Mutex::new(None));
        let worker = MockWorker {
            stopped: Arc::clone(&worker_stopped),
            stop_timeout: Arc::clone(&stop_timeout),
        };
        let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(
            DummyStore,
            DummyDownloader,
            ModelOrchestratorConfig {
                model_url: "url".to_string(),
                expected_sha256: "hash".to_string(),
            },
        )));
        let orch = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
            worker,
            DummyCtx,
            model_orch,
            Duration::from_secs(5),
        )));
        orch.lock().unwrap().ensure_model().expect("ensure model");
        let transcribe_hook = Arc::new(TranscribeLifecycleHook::new(
            orch.clone() as Arc<Mutex<dyn TranscribeOrchestrator>>,
            Arc::new(DummyEmitter),
        ));
        transcribe_hook.on_capture_phase_changed(CapturePhase::Capturing);
        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);

        let (_, _, capture_orch) = make_orchestrator();
        let state = CaptureLifecycleState::new(
            Box::new(capture_orch),
            Arc::new(FixedPlatformSupport { supported: true }),
            Arc::new(RecordingUnsupportedPlatformNotifier::new()),
        );

        perform_app_exit_shutdown(Some(&transcribe_hook), &state);

        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
        assert!(*worker_stopped.lock().unwrap());
        assert_eq!(*stop_timeout.lock().unwrap(), Some(Duration::from_secs(5)));
    }
}
