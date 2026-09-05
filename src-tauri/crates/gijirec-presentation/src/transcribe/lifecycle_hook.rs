//! Transcribe lifecycle hook connecting audio-capture states & app events to TranscribeOrchestrator.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gijirec_application::transcribe::TranscribeOrchestrator;
use gijirec_domain::audio::CapturePhase;
use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};

use crate::tauri::lifecycle::CaptureProcessingHook;
use crate::transcribe::event_emitter::TranscribeEventEmitter;

/// Default join timeout when stopping transcribe worker on lifecycle events.
pub const DEFAULT_TRANSCRIBE_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// Hooks capture events and app termination into [`TranscribeOrchestrator`].
pub struct TranscribeLifecycleHook {
    orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    emitter: Mutex<Arc<dyn TranscribeEventEmitter>>,
}

impl TranscribeLifecycleHook {
    /// Creates a new `TranscribeLifecycleHook`.
    pub fn new(
        orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
        emitter: Arc<dyn TranscribeEventEmitter>,
    ) -> Self {
        Self {
            orchestrator,
            emitter: Mutex::new(emitter),
        }
    }

    /// Sets the real event emitter when Tauri AppHandle is initialized.
    pub fn set_emitter(&self, emitter: Arc<dyn TranscribeEventEmitter>) {
        if let Ok(mut lock) = self.emitter.lock() {
            *lock = emitter;
        }
    }

    fn emitter(&self) -> Arc<dyn TranscribeEventEmitter> {
        self.emitter.lock().expect("lock emitter").clone()
    }

    /// Handles capture phase change notification (`audio-capture://phase-changed`).
    pub fn on_capture_phase_changed(&self, phase: CapturePhase) {
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        match phase {
            CapturePhase::Capturing => {
                // Set upstream capturing to true so start gate can pass
                orch.set_upstream_capturing(true);
                // If model is ready, start transcription
                if orch.phase() == TranscribePhase::Ready
                    && let Ok(()) = orch.start()
                {
                    let _ = emitter.emit_phase_changed(orch.phase());
                }
            }
            CapturePhase::Error => {
                // Upstream capture error: stop processing new PCM, emit UPSTREAM_CAPTURE_ERROR
                orch.on_upstream_capture_error();
                let _ = emitter.emit_error(&TranscribeError::UpstreamCaptureError);
                let _ = emitter.emit_phase_changed(orch.phase());
            }
            CapturePhase::Stopping | CapturePhase::Idle => {
                orch.pause_capture();
                let _ = emitter.emit_phase_changed(orch.phase());
            }
            _ => {}
        }
    }

    /// Handles application shutdown or window close.
    pub fn on_app_exit(&self) {
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.set_upstream_capturing(false);
        if orch.phase() == TranscribePhase::Transcribing
            || orch.phase() == TranscribePhase::LoadingModel
        {
            let _ = orch.stop();
            let _ = emitter.emit_phase_changed(orch.phase());
        }
    }
}

impl CaptureProcessingHook for TranscribeLifecycleHook {
    fn on_capture_started(&self) {
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.set_upstream_capturing(true);
        if orch.phase() == TranscribePhase::Ready
            && let Ok(()) = orch.start()
        {
            let _ = emitter.emit_phase_changed(orch.phase());
        }
    }

    fn on_capture_stopping(&self) {
        let emitter = self.emitter();
        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.set_upstream_capturing(false);
        if orch.phase() == TranscribePhase::Transcribing {
            let _ = orch.stop();
            let _ = emitter.emit_phase_changed(orch.phase());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_application::transcribe::ModelDownloadProgress;
    use gijirec_domain::transcribe::{TranscribeErrorCode, UserFacingTranscribeError};

    struct MockOrchestrator {
        phase: TranscribePhase,
        upstream_capturing: bool,
        start_count: u32,
        stop_count: u32,
        upstream_error_count: u32,
    }

    impl MockOrchestrator {
        fn new(initial_phase: TranscribePhase) -> Self {
            Self {
                phase: initial_phase,
                upstream_capturing: false,
                start_count: 0,
                stop_count: 0,
                upstream_error_count: 0,
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

    #[test]
    fn on_capture_started_sets_upstream_and_starts_orchestrator_when_ready() {
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
    }

    #[test]
    fn on_capture_stopping_stops_orchestrator_when_transcribing() {
        let mut o = MockOrchestrator::new(TranscribePhase::Ready);
        o.set_upstream_capturing(true);
        o.start().unwrap();

        let orch = Arc::new(Mutex::new(o));
        let emitter = Arc::new(MockEmitter::default());
        let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

        hook.on_capture_stopping();

        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
        assert_eq!(orch.lock().unwrap().stop_count, 1);
        assert!(!orch.lock().unwrap().upstream_capturing);
        assert_eq!(*emitter.phases.lock().unwrap(), vec![TranscribePhase::Idle]);
    }

    #[test]
    fn upstream_capture_error_notifies_orchestrator_and_capturing_resumes() {
        let mut o = MockOrchestrator::new(TranscribePhase::Ready);
        o.set_upstream_capturing(true);
        o.start().unwrap();

        let orch = Arc::new(Mutex::new(o));
        let emitter = Arc::new(MockEmitter::default());
        let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

        // 1. Upstream capture error happens
        hook.on_capture_phase_changed(CapturePhase::Error);

        assert_eq!(orch.lock().unwrap().upstream_error_count, 1);
        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
        assert!(!orch.lock().unwrap().upstream_capturing);
        let errors = emitter.errors.lock().unwrap().clone();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, TranscribeErrorCode::UpstreamCaptureError);

        // 2. Upstream capture resumes (Capturing)
        hook.on_capture_phase_changed(CapturePhase::Capturing);

        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
        assert!(orch.lock().unwrap().upstream_capturing);
        let phases = emitter.phases.lock().unwrap().clone();
        assert_eq!(
            phases,
            vec![TranscribePhase::Ready, TranscribePhase::Transcribing]
        );
    }

    #[test]
    fn capture_pause_via_phase_changed_stopping_transitions_to_ready_and_can_restart() {
        let mut o = MockOrchestrator::new(TranscribePhase::Ready);
        o.set_upstream_capturing(true);
        o.start().unwrap();

        let orch = Arc::new(Mutex::new(o));
        let emitter = Arc::new(MockEmitter::default());
        let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

        // 1. Capture stops/pauses
        hook.on_capture_phase_changed(CapturePhase::Stopping);

        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
        assert_eq!(orch.lock().unwrap().stop_count, 1);
        assert!(!orch.lock().unwrap().upstream_capturing);

        // 2. Capture starts again in same session
        hook.on_capture_phase_changed(CapturePhase::Capturing);

        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
        assert_eq!(orch.lock().unwrap().start_count, 2);
    }

    #[test]
    fn on_app_exit_stops_transcribing_worker() {
        let mut o = MockOrchestrator::new(TranscribePhase::Ready);
        o.set_upstream_capturing(true);
        o.start().unwrap();

        let orch = Arc::new(Mutex::new(o));
        let emitter = Arc::new(MockEmitter::default());
        let hook = TranscribeLifecycleHook::new(orch.clone(), emitter.clone());

        hook.on_app_exit();

        assert_eq!(orch.lock().unwrap().stop_count, 1);
        assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Idle);
    }
}
