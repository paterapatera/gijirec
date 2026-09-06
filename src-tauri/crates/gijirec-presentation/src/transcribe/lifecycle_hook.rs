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
    OrchestratorStallAdapter, SharedTranscribeEmitter, StallClock, StallWatchdogRuntime,
    TranscribeStallWatchdog,
};

/// Default join timeout when stopping transcribe worker on lifecycle events.
pub const DEFAULT_TRANSCRIBE_STOP_TIMEOUT: Duration = Duration::from_secs(5);

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

    fn start_transcribing_if_ready(&self) {
        let emitter = self.emitter();
        let started_transcribing = {
            let mut orch = self.orchestrator.lock().expect("lock orchestrator");
            orch.set_upstream_capturing(true);
            if orch.phase() == TranscribePhase::Ready
                && let Ok(()) = orch.start()
            {
                let phase = orch.phase();
                self.emit_phase(emitter.as_ref(), phase);
                phase == TranscribePhase::Transcribing
            } else {
                false
            }
        };
        if started_transcribing {
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
        let emitter = self.emitter();
        let started_transcribing = {
            let mut orch = self.orchestrator.lock().expect("lock orchestrator");
            if orch.phase() == TranscribePhase::Ready
                && let Ok(()) = orch.start()
            {
                let phase = orch.phase();
                self.emit_phase(emitter.as_ref(), phase);
                phase == TranscribePhase::Transcribing
            } else {
                false
            }
        };
        if started_transcribing {
            self.start_stall_watchdog();
        }
    }

    fn pause_transcribing_for_capture_stop(&self) {
        self.stop_stall_watchdog();
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.pause_capture();
        self.emit_phase(emitter.as_ref(), orch.phase());
    }

    fn stop_transcribing_worker(&self) {
        self.stop_stall_watchdog();
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.set_upstream_capturing(false);
        if orch.phase() == TranscribePhase::Transcribing {
            let _ = orch.stop();
            self.emit_phase(emitter.as_ref(), orch.phase());
        }
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
                self.stop_stall_watchdog();
                let mut orch = self.orchestrator.lock().expect("lock orchestrator");
                orch.pause_capture();
                self.emit_phase(emitter.as_ref(), orch.phase());
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
    use gijirec_application::transcribe::ModelDownloadProgress;
    use gijirec_domain::transcribe::{TranscribeErrorCode, UserFacingTranscribeError};
    use std::sync::atomic::{AtomicU64, Ordering};

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

    #[derive(Default)]
    struct MockEmitter {
        phases: Mutex<Vec<TranscribePhase>>,
        errors: Mutex<Vec<UserFacingTranscribeError>>,
    }

    impl TranscribeEventEmitter for MockEmitter {
        fn emit_phase_changed(
            &self,
            phase: TranscribePhase,
        ) -> Result<(), crate::transcribe::event_emitter::TranscribeEmitError> {
            self.phases.lock().unwrap().push(phase);
            Ok(())
        }

        fn emit_model_progress(
            &self,
            _progress: &ModelDownloadProgress,
        ) -> Result<(), crate::transcribe::event_emitter::TranscribeEmitError> {
            Ok(())
        }

        fn emit_error(
            &self,
            error: &TranscribeError,
        ) -> Result<(), crate::transcribe::event_emitter::TranscribeEmitError> {
            self.errors.lock().unwrap().push(error.to_user_facing());
            Ok(())
        }
    }

    fn test_clock() -> StallClock {
        let time = Arc::new(AtomicU64::new(0));
        Arc::new(move || time.load(Ordering::SeqCst))
    }

    #[test]
    fn on_capture_started_sets_upstream_and_starts_orchestrator_when_ready() {
        with_isolated_transcribe_observability(|| {
            let orch = Arc::new(Mutex::new(MockOrchestrator::new(TranscribePhase::Ready)));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

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
            let mut o = MockOrchestrator::new(TranscribePhase::Ready);
            o.set_upstream_capturing(true);
            o.start().unwrap();

            let orch = Arc::new(Mutex::new(o));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

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
            let orch = Arc::new(Mutex::new(MockOrchestrator::new(TranscribePhase::Ready)));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::with_stall_watchdog(
                orch.clone(),
                emitter.clone(),
                test_clock(),
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
            let mut o = MockOrchestrator::new(TranscribePhase::Ready);
            o.set_upstream_capturing(true);
            o.start().unwrap();

            let orch = Arc::new(Mutex::new(o));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

            hook.on_worker_engine_failed(TranscribeError::ModelCorrupt {
                detail: "load failed".to_string(),
            });

            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Error);
            assert_eq!(orch.lock().unwrap().fail_inference_count, 1);
            let errors = emitter.errors.lock().unwrap().clone();
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
            let mut o = MockOrchestrator::new(TranscribePhase::Ready);
            o.set_upstream_capturing(true);
            o.start().unwrap();

            let orch = Arc::new(Mutex::new(o));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

            hook.on_capture_phase_changed(CapturePhase::Error);

            assert_eq!(orch.lock().unwrap().upstream_error_count, 1);
            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
            assert!(!orch.lock().unwrap().upstream_capturing);
            let errors = emitter.errors.lock().unwrap().clone();
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
            let mut o = MockOrchestrator::new(TranscribePhase::Ready);
            o.set_upstream_capturing(true);
            o.start().unwrap();

            let orch = Arc::new(Mutex::new(o));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

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
    fn on_app_exit_stops_transcribing_worker() {
        with_isolated_transcribe_observability(|| {
            let mut o = MockOrchestrator::new(TranscribePhase::Ready);
            o.set_upstream_capturing(true);
            o.start().unwrap();

            let orch = Arc::new(Mutex::new(o));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

            hook.on_app_exit();

            assert_eq!(orch.lock().unwrap().stop_count, 1);
            assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
        });
    }

    #[test]
    fn on_capture_started_logs_phase_transition_via_observability() {
        let recorder = RecordingTranscribeObservability::new();
        with_test_transcribe_observability(&recorder, || {
            let orch = Arc::new(Mutex::new(MockOrchestrator::new(TranscribePhase::Ready)));
            let emitter = Arc::new(MockEmitter::default());
            let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());
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
            let orch = Arc::new(Mutex::new(MockOrchestrator::new(TranscribePhase::Ready)));
            let emitter = Arc::new(MockEmitter::default());
            let hook =
                TranscribeLifecycleHook::with_stall_watchdog(orch.clone(), emitter, test_clock());
            let watchdog = hook
                .stall_watchdog()
                .expect("stall watchdog must be configured");

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
            let mut o = MockOrchestrator::new(TranscribePhase::Ready);
            o.set_upstream_capturing(true);
            o.start().unwrap();

            let orch = Arc::new(Mutex::new(o));
            let emitter = Arc::new(MockEmitter::default());
            let hook =
                TranscribeLifecycleHook::with_stall_watchdog(orch.clone(), emitter, test_clock());
            let watchdog = hook
                .stall_watchdog()
                .expect("stall watchdog must be configured");
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
