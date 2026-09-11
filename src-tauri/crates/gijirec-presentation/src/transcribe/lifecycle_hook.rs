//! Transcribe lifecycle hook connecting audio-capture states & app events to TranscribeOrchestrator.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gijirec_application::transcribe::TranscribeOrchestrator;
use gijirec_domain::audio::CapturePhase;
use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};

use crate::tauri::lifecycle::CaptureProcessingHook;
use crate::transcribe::event_emitter::TranscribeEventEmitter;
use crate::transcribe::observability;
use crate::transcribe::stall_watchdog::{
    BATCH_INTERVAL, OrchestratorStallAdapter, SharedTranscribeEmitter, StallClock,
    StallWatchdogRuntime, TranscribeStallWatchdog,
};

/// Headroom for one 30 s batch window encode during worker stop+flush join.
pub const TRANSCRIBE_STOP_INFERENCE_MARGIN: Duration = Duration::from_secs(30);

/// Default join timeout when stopping transcribe worker on lifecycle events.
///
/// Must cover [`transcribe_worker::stop_and_join`] stop-flush (remaining PCM as final batch).
/// Aligns with task 2.3 worker shutdown path used by both capture pause and app exit.
/// Equals [`BATCH_INTERVAL`] + [`TRANSCRIBE_STOP_INFERENCE_MARGIN`] (60 s).
pub const DEFAULT_TRANSCRIBE_STOP_TIMEOUT: Duration =
    Duration::from_secs(BATCH_INTERVAL.as_secs() + TRANSCRIBE_STOP_INFERENCE_MARGIN.as_secs());

type StallWatchdogHandle =
    TranscribeStallWatchdog<OrchestratorStallAdapter, SharedTranscribeEmitter>;

struct StallWatchdogBundle {
    adapter: Arc<Mutex<OrchestratorStallAdapter>>,
    watchdog: Arc<StallWatchdogHandle>,
    runtime: StallWatchdogRuntime,
}

/// Hooks capture events and app termination into [`TranscribeOrchestrator`].
pub struct TranscribeLifecycleHook {
    orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    emitter: Arc<Mutex<Arc<dyn TranscribeEventEmitter>>>,
    stall: Option<StallWatchdogBundle>,
}

impl TranscribeLifecycleHook {
    /// Creates a new `TranscribeLifecycleHook` without stall watchdog wiring.
    pub fn new(
        orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
        emitter: Arc<dyn TranscribeEventEmitter>,
    ) -> Self {
        Self {
            orchestrator,
            emitter: Arc::new(Mutex::new(emitter)),
            stall: None,
        }
    }

    /// Creates a hook with stall watchdog polling enabled for capture/transcribe sessions.
    pub fn with_stall_watchdog(
        orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
        emitter: Arc<dyn TranscribeEventEmitter>,
        clock: StallClock,
    ) -> Self {
        let emitter_cell = Arc::new(Mutex::new(emitter));
        let adapter = Arc::new(Mutex::new(OrchestratorStallAdapter::new(Arc::clone(
            &orchestrator,
        ))));
        let watchdog = Arc::new(TranscribeStallWatchdog::new(
            Arc::clone(&adapter),
            Arc::new(SharedTranscribeEmitter::new(Arc::clone(&emitter_cell))),
            clock,
        ));
        Self {
            orchestrator,
            emitter: emitter_cell,
            stall: Some(StallWatchdogBundle {
                adapter,
                watchdog,
                runtime: StallWatchdogRuntime::new(),
            }),
        }
    }

    /// Sets the real event emitter when Tauri AppHandle is initialized.
    pub fn set_emitter(&self, emitter: Arc<dyn TranscribeEventEmitter>) {
        if let Ok(mut lock) = self.emitter.lock() {
            *lock = emitter;
        }
    }

    /// Returns the stall watchdog when lifecycle wiring is enabled.
    pub fn stall_watchdog(&self) -> Option<Arc<StallWatchdogHandle>> {
        self.stall
            .as_ref()
            .map(|bundle| Arc::clone(&bundle.watchdog))
    }

    fn emitter(&self) -> Arc<dyn TranscribeEventEmitter> {
        self.emitter.lock().expect("lock emitter").clone()
    }

    fn emit_phase(&self, emitter: &dyn TranscribeEventEmitter, phase: TranscribePhase) {
        observability::log_phase_transition(phase);
        let _ = emitter.emit_phase_changed(phase);
    }

    fn emit_error(&self, emitter: &dyn TranscribeEventEmitter, error: &TranscribeError) {
        observability::log_transcribe_error(error);
        let _ = emitter.emit_error(error);
    }

    fn start_stall_watchdog(&self) {
        if let Some(bundle) = &self.stall {
            bundle
                .adapter
                .lock()
                .expect("lock adapter")
                .set_upstream_capturing(true);
            bundle.watchdog.arm();
            bundle.runtime.start(Arc::clone(&bundle.watchdog));
        }
    }

    fn stop_stall_watchdog(&self) {
        if let Some(bundle) = &self.stall {
            bundle.runtime.stop();
            bundle.watchdog.disarm();
            bundle
                .adapter
                .lock()
                .expect("lock adapter")
                .set_upstream_capturing(false);
        }
    }

    fn try_start_transcribing(
        &self,
        prepare: impl FnOnce(&mut dyn TranscribeOrchestrator),
    ) -> bool {
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        prepare(&mut *orch);
        if orch.phase() == TranscribePhase::Ready && orch.start().is_ok() {
            let phase = orch.phase();
            self.emit_phase(emitter.as_ref(), phase);
            phase == TranscribePhase::Transcribing
        } else {
            false
        }
    }

    fn start_transcribing_if_ready(&self) {
        if self.try_start_transcribing(|orch| orch.set_upstream_capturing(true)) {
            self.start_stall_watchdog();
        }
    }

    /// Surfaces a worker-thread engine failure (load/init) that would otherwise be silent.
    pub fn on_worker_engine_failed(&self, error: TranscribeError) {
        self.stop_stall_watchdog();
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.fail_inference();
        self.emit_error(emitter.as_ref(), &error);
        self.emit_phase(emitter.as_ref(), orch.phase());
    }

    /// Starts transcription after the model becomes ready if capture is already active.
    /// Does not mark upstream capturing by itself — that remains the capture hook's job.
    pub fn on_model_ready(&self) {
        if self.try_start_transcribing(|_| {}) {
            self.start_stall_watchdog();
        }
    }

    /// Stops the worker with PCM flush via orchestrator (same path as batch worker `stop_and_join`).
    fn stop_transcribing_with_flush(&self, flush_via_stop: bool) {
        self.stop_stall_watchdog();
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.set_upstream_capturing(false);
        if orch.phase() == TranscribePhase::Transcribing {
            if flush_via_stop {
                let _ = orch.stop();
            } else {
                orch.pause_capture();
            }
            self.emit_phase(emitter.as_ref(), orch.phase());
        }
    }

    fn pause_transcribing_for_capture_stop(&self) {
        self.stop_transcribing_with_flush(false);
    }

    fn stop_transcribing_worker(&self) {
        self.stop_transcribing_with_flush(true);
    }

    /// Handles capture phase change notification (`audio-capture://phase-changed`).
    pub fn on_capture_phase_changed(&self, phase: CapturePhase) {
        let emitter = self.emitter();
        match phase {
            CapturePhase::Capturing => self.start_transcribing_if_ready(),
            CapturePhase::Error => {
                self.stop_stall_watchdog();
                let mut orch = self.orchestrator.lock().expect("lock orchestrator");
                orch.on_upstream_capture_error();
                self.emit_error(emitter.as_ref(), &TranscribeError::UpstreamCaptureError);
                self.emit_phase(emitter.as_ref(), orch.phase());
            }
            CapturePhase::Stopping | CapturePhase::Idle => {
                self.stop_transcribing_with_flush(false);
            }
            _ => {}
        }
    }

    /// Handles application shutdown or window close.
    pub fn on_app_exit(&self) {
        self.stop_transcribing_worker();
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        if orch.phase() == TranscribePhase::LoadingModel {
            let _ = orch.stop();
            self.emit_phase(emitter.as_ref(), orch.phase());
        }
    }
}

impl CaptureProcessingHook for TranscribeLifecycleHook {
    fn on_capture_started(&self) {
        self.start_transcribing_if_ready();
    }

    fn on_capture_stopping(&self) {
        self.pause_transcribing_for_capture_stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::observability::{
        RecordingTranscribeObservability, with_isolated_transcribe_observability,
        with_test_transcribe_observability,
    };
    use crate::transcribe::test_support::{MockTranscribeEmitter, stall_clock_only};
    use gijirec_application::transcribe::ModelDownloadProgress;
    use gijirec_domain::transcribe::TranscribeErrorCode;

    struct MockOrchestrator {
        phase: TranscribePhase,
        upstream_capturing: bool,
        start_count: u32,
        stop_count: u32,
        upstream_error_count: u32,
        fail_inference_count: u32,
    }

    impl MockOrchestrator {
        fn new(initial_phase: TranscribePhase) -> Self {
            Self {
                phase: initial_phase,
                upstream_capturing: false,
                start_count: 0,
                stop_count: 0,
                upstream_error_count: 0,
                fail_inference_count: 0,
            }
        }
    }

    impl TranscribeOrchestrator for MockOrchestrator {
        fn ensure_model(&mut self) -> Result<(), TranscribeError> {
            self.phase = TranscribePhase::Ready;
            Ok(())
        }

        fn begin_model_loading(&mut self) -> Result<(), TranscribeError> {
            self.phase = TranscribePhase::LoadingModel;
            Ok(())
        }

        fn finish_model_loading(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
            self.phase = TranscribePhase::Ready;
            Ok(())
        }

        fn fail_model_loading(&mut self) {
            self.phase = TranscribePhase::Error;
        }

        fn set_model_progress_callback(
            &mut self,
            _callback: Box<dyn FnMut(ModelDownloadProgress) + Send>,
        ) {
        }

        fn start(&mut self) -> Result<(), TranscribeError> {
            if self.phase == TranscribePhase::Ready && self.upstream_capturing {
                self.phase = TranscribePhase::Transcribing;
                self.start_count += 1;
                Ok(())
            } else {
                Err(TranscribeError::Internal {
                    detail: "cannot start without ready phase and upstream capturing".to_string(),
                })
            }
        }

        fn stop(&mut self) -> Result<(), TranscribeError> {
            self.phase = TranscribePhase::Idle;
            self.stop_count += 1;
            Ok(())
        }

        fn pause_capture(&mut self) {
            self.upstream_capturing = false;
            if self.phase == TranscribePhase::Transcribing {
                self.phase = TranscribePhase::Ready;
                self.stop_count += 1;
            }
        }

        fn phase(&self) -> TranscribePhase {
            self.phase
        }

        fn on_upstream_capture_error(&mut self) {
            self.upstream_capturing = false;
            self.upstream_error_count += 1;
            self.phase = TranscribePhase::Ready;
        }

        fn set_upstream_capturing(&mut self, capturing: bool) {
            self.upstream_capturing = capturing;
        }

        fn fail_inference(&mut self) {
            self.fail_inference_count += 1;
            self.upstream_capturing = false;
            self.phase = TranscribePhase::Error;
        }
    }

    type HookFixture = (
        Arc<Mutex<MockOrchestrator>>,
        Arc<MockTranscribeEmitter>,
        TranscribeLifecycleHook,
    );

    fn hook_fixture(orch: MockOrchestrator) -> HookFixture {
        let orch = Arc::new(Mutex::new(orch));
        let emitter = Arc::new(MockTranscribeEmitter::default());
        let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());
        (orch, emitter, hook)
    }

    fn ready_hook() -> HookFixture {
        hook_fixture(MockOrchestrator::new(TranscribePhase::Ready))
    }

    fn transcribing_hook() -> HookFixture {
        let mut orch = MockOrchestrator::new(TranscribePhase::Ready);
        orch.set_upstream_capturing(true);
        orch.start().unwrap();
        hook_fixture(orch)
    }

    fn stall_watchdog_fixture(
        fixture: HookFixture,
    ) -> (
        Arc<Mutex<MockOrchestrator>>,
        TranscribeLifecycleHook,
        Arc<StallWatchdogHandle>,
    ) {
        let (orch, emitter, _) = fixture;
        let hook =
            TranscribeLifecycleHook::with_stall_watchdog(orch.clone(), emitter, stall_clock_only());
        let watchdog = hook
            .stall_watchdog()
            .expect("stall watchdog must be configured");
        (orch, hook, watchdog)
    }

    #[test]
    fn default_stop_timeout_accommodates_batch_flush() {
        assert_eq!(DEFAULT_TRANSCRIBE_STOP_TIMEOUT, Duration::from_secs(60));
        assert!(
            DEFAULT_TRANSCRIBE_STOP_TIMEOUT >= BATCH_INTERVAL,
            "lifecycle stop join must cover at least one 30 s batch flush window"
        );
        assert_eq!(
            DEFAULT_TRANSCRIBE_STOP_TIMEOUT.as_secs(),
            TRANSCRIBE_STOP_INFERENCE_MARGIN.as_secs() + BATCH_INTERVAL.as_secs()
        );
    }

    #[test]
    fn capture_stop_triggers_worker_stop_for_pcm_flush() {
        with_isolated_transcribe_observability(|| {
            let (orch, _emitter, hook) = transcribing_hook();

            hook.on_capture_stopping();

            assert_eq!(
                orch.lock().unwrap().stop_count,
                1,
                "capture stop must stop worker for PCM flush"
            );
            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
            assert!(!orch.lock().unwrap().upstream_capturing);
        });
    }

    #[test]
    fn on_capture_started_sets_upstream_and_starts_orchestrator_when_ready() {
        with_isolated_transcribe_observability(|| {
            let (orch, emitter, hook) = ready_hook();

            hook.on_capture_started();

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
            assert_eq!(orch.lock().unwrap().start_count, 1);
            assert_eq!(
                *emitter.phases.lock().unwrap(),
                vec![TranscribePhase::Transcribing]
            );
        });
    }

    #[test]
    fn on_capture_stopping_pauses_to_ready_when_transcribing() {
        with_isolated_transcribe_observability(|| {
            let (orch, emitter, hook) = transcribing_hook();

            hook.on_capture_stopping();

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
            assert_eq!(orch.lock().unwrap().stop_count, 1);
            assert!(!orch.lock().unwrap().upstream_capturing);
            assert_eq!(
                *emitter.phases.lock().unwrap(),
                vec![TranscribePhase::Ready]
            );
        });
    }

    #[test]
    fn on_model_ready_starts_and_arms_watchdog_when_capture_already_active() {
        with_isolated_transcribe_observability(|| {
            let (orch, emitter, _) = ready_hook();
            let hook = TranscribeLifecycleHook::with_stall_watchdog(
                orch.clone(),
                emitter.clone(),
                stall_clock_only(),
            );
            orch.lock().unwrap().set_upstream_capturing(true);

            hook.on_model_ready();

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
            assert_eq!(orch.lock().unwrap().start_count, 1);
            assert!(hook.stall_watchdog().expect("stall watchdog").is_armed());
            assert_eq!(
                *emitter.phases.lock().unwrap(),
                vec![TranscribePhase::Transcribing]
            );
        });
    }

    #[test]
    fn on_worker_engine_failed_emits_error_and_error_phase() {
        with_isolated_transcribe_observability(|| {
            let (orch, emitter, hook) = transcribing_hook();

            hook.on_worker_engine_failed(TranscribeError::ModelCorrupt {
                detail: "load failed".to_string(),
            });

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Error);
            assert_eq!(orch.lock().unwrap().fail_inference_count, 1);
            let errors = emitter.user_errors.lock().unwrap().clone();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, TranscribeErrorCode::ModelCorrupt);
            assert_eq!(
                *emitter.phases.lock().unwrap(),
                vec![TranscribePhase::Error]
            );
        });
    }

    #[test]
    fn upstream_capture_error_notifies_orchestrator_and_capturing_resumes() {
        with_isolated_transcribe_observability(|| {
            let (orch, emitter, hook) = transcribing_hook();

            hook.on_capture_phase_changed(CapturePhase::Error);

            assert_eq!(orch.lock().unwrap().upstream_error_count, 1);
            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
            assert!(!orch.lock().unwrap().upstream_capturing);
            let errors = emitter.user_errors.lock().unwrap().clone();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, TranscribeErrorCode::UpstreamCaptureError);

            hook.on_capture_phase_changed(CapturePhase::Capturing);

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
            assert!(orch.lock().unwrap().upstream_capturing);
            let phases = emitter.phases.lock().unwrap().clone();
            assert_eq!(
                phases,
                vec![TranscribePhase::Ready, TranscribePhase::Transcribing]
            );
        });
    }

    #[test]
    fn capture_pause_via_phase_changed_stopping_transitions_to_ready_and_can_restart() {
        with_isolated_transcribe_observability(|| {
            let (orch, _emitter, hook) = transcribing_hook();

            hook.on_capture_phase_changed(CapturePhase::Stopping);

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
            assert_eq!(orch.lock().unwrap().stop_count, 1);
            assert!(!orch.lock().unwrap().upstream_capturing);

            hook.on_capture_phase_changed(CapturePhase::Capturing);

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
            assert_eq!(orch.lock().unwrap().start_count, 2);
        });
    }

    #[test]
    fn on_app_exit_stops_transcribing_worker_and_loading_model() {
        with_isolated_transcribe_observability(|| {
            let (orch, _emitter, hook) = transcribing_hook();

            hook.on_app_exit();

            assert_eq!(orch.lock().unwrap().stop_count, 1);
            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
            assert!(!orch.lock().unwrap().upstream_capturing);

            let mut loading = MockOrchestrator::new(TranscribePhase::LoadingModel);
            loading.set_upstream_capturing(true);
            let orch_loading = Arc::new(Mutex::new(loading));
            let hook_loading = TranscribeLifecycleHook::new(
                orch_loading.clone(),
                Arc::new(MockTranscribeEmitter::default()),
            );

            hook_loading.on_app_exit();

            assert_eq!(orch_loading.lock().unwrap().stop_count, 1);
            assert_eq!(orch_loading.lock().unwrap().phase(), TranscribePhase::Idle);
        });
    }

    #[test]
    fn on_capture_started_logs_phase_transition_via_observability() {
        let recorder = RecordingTranscribeObservability::new();
        with_test_transcribe_observability(&recorder, || {
            let (_orch, _emitter, hook) = ready_hook();
            hook.on_capture_started();
        });

        assert_eq!(
            *recorder.phases.lock().expect("lock"),
            vec![TranscribePhase::Transcribing]
        );
    }

    #[test]
    fn with_stall_watchdog_arms_on_transcribing_and_disarms_on_capture_stop() {
        with_isolated_transcribe_observability(|| {
            let (orch, hook, watchdog) = stall_watchdog_fixture(ready_hook());

            hook.on_capture_started();
            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
            assert!(watchdog.is_armed());

            hook.on_capture_stopping();
            assert!(!watchdog.is_armed());
        });
    }

    #[test]
    fn with_stall_watchdog_disarms_on_capture_error_and_pause() {
        with_isolated_transcribe_observability(|| {
            let (_orch, hook, watchdog) = stall_watchdog_fixture(transcribing_hook());
            watchdog.arm();

            hook.on_capture_phase_changed(CapturePhase::Error);
            assert!(!watchdog.is_armed());

            hook.on_capture_phase_changed(CapturePhase::Capturing);
            assert!(watchdog.is_armed());

            hook.on_capture_phase_changed(CapturePhase::Stopping);
            assert!(!watchdog.is_armed());
        });
    }
}
