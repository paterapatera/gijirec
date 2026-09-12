//! Tauri lifecycle hooks: sync app startup/shutdown with capture orchestration.

use crate::tauri::events::CaptureEventEmitter;
use crate::tauri::observability;
use crate::transcribe::TranscribeLifecycleHook;
use gijirec_application::capture::orchestrator::CaptureOrchestrator;
use gijirec_application::device_selection::DeviceSelectionService;
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
#[allow(clippy::too_many_arguments)]
pub fn on_app_setup(
    platform: &dyn CapturePlatformSupport,
    orchestrator: &mut dyn CaptureOrchestrator,
    selection: &dyn DeviceSelectionService,
    emitter: &dyn CaptureEventEmitter,
    notifier: &dyn UnsupportedPlatformNotifier,
) -> Result<(), LifecycleError> {
    if !platform.is_capture_supported() {
        notifier.notify_unsupported();
        return Ok(());
    }

    let startup_selection = selection.get_selection();

    if orchestrator.phase() == CapturePhase::Idle {
        emit_phase(emitter, CapturePhase::Starting)?;
    }

    match orchestrator.start_with_selection(&startup_selection) {
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

/// Handles runtime stream disconnect while capturing: stop processing, error phase, emit (req 4.3).
pub fn handle_capture_device_disconnected(
    orchestrator: &mut dyn CaptureOrchestrator,
    emitter: &dyn CaptureEventEmitter,
    processing: Option<&dyn CaptureProcessingHook>,
) -> Result<(), LifecycleError> {
    if orchestrator.phase() != CapturePhase::Capturing {
        return Ok(());
    }

    if let Some(hook) = processing {
        hook.on_capture_stopping();
    }

    match orchestrator.on_device_disconnected() {
        Ok(()) => Ok(()),
        Err(CaptureError::DeviceDisconnected) => {
            emit_error(emitter, CaptureError::DeviceDisconnected)?;
            emit_phase(emitter, CapturePhase::Error)?;
            Ok(())
        }
        Err(err) => Err(LifecycleError::Orchestrator(err)),
    }
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
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    device_selection: Arc<dyn DeviceSelectionService>,
    emitter: Mutex<Option<Arc<dyn CaptureEventEmitter>>>,
    platform: Arc<dyn CapturePlatformSupport>,
    notifier: Arc<dyn UnsupportedPlatformNotifier>,
    processing: Mutex<Option<Arc<dyn CaptureProcessingHook>>>,
}

impl CaptureLifecycleState {
    pub fn new(
        orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
        device_selection: Arc<dyn DeviceSelectionService>,
        platform: Arc<dyn CapturePlatformSupport>,
        notifier: Arc<dyn UnsupportedPlatformNotifier>,
    ) -> Self {
        Self {
            orchestrator,
            device_selection,
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

    /// Invoked when a capture stream reports disconnect/error during `capturing` (req 4.3).
    pub fn handle_stream_disconnected(&self) {
        if !self.platform.is_capture_supported() {
            return;
        }
        let Some(emitter) = self.emitter() else {
            return;
        };
        let mut orchestrator = self.orchestrator.lock().expect("lock");
        let processing = self.processing.lock().expect("lock");
        let hook = processing
            .as_ref()
            .map(|arc| arc.as_ref() as &dyn CaptureProcessingHook);
        let _ = handle_capture_device_disconnected(&mut *orchestrator, emitter.as_ref(), hook);
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
        let _ = on_app_shutdown(&mut *orch, emitter.as_ref());
    }
}

pub(crate) fn handle_window_close_requested<R: Runtime>(app: &AppHandle<R>) {
    let transcribe_hook = app.try_state::<Arc<TranscribeLifecycleHook>>();
    let state = app.state::<Arc<CaptureLifecycleState>>();
    perform_app_exit_shutdown(transcribe_hook.as_ref().map(|h| &***h), state.as_ref());
}

/// Initializes capture emitter and starts capture on app setup.
///
/// Tauri allows only one `.setup()` callback; call this from the composition root's
/// unified setup instead of registering a second handler.
///
/// Capture startup failures (e.g. missing screen-recording permission) are emitted to
/// the frontend as `error` phase events; they must not abort Tauri setup.
pub fn run_capture_app_setup<R: Runtime>(app: &AppHandle<R>) -> Result<(), LifecycleError> {
    let managed = app.state::<Arc<CaptureLifecycleState>>();
    managed.init_emitter(Arc::new(
        crate::tauri::events::TauriCaptureEventEmitter::new(app.clone()),
    ));
    let emitter = managed
        .emitter()
        .expect("emitter must be initialized in setup");
    let mut orch = managed.orchestrator.lock().expect("lock");
    if let Err(LifecycleError::Orchestrator(_)) = on_app_setup(
        managed.platform.as_ref(),
        &mut *orch,
        managed.device_selection.as_ref(),
        emitter.as_ref(),
        managed.notifier.as_ref(),
    ) {
        // User-facing error already emitted; keep the app window open for recovery.
    }

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
    state: Arc<CaptureLifecycleState>,
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
        let state = app.state::<Arc<CaptureLifecycleState>>();
        perform_app_exit_shutdown(transcribe_hook.as_ref().map(|h| &***h), state.as_ref());
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
