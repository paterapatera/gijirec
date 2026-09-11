//! Transcribe lifecycle orchestration with phase gates and upstream capture coupling.

use std::marker::PhantomData;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};

use super::model_orchestrator::ModelOrchestrator;
use super::ports::{ModelDownloadProgress, TranscribeWorkerPort, WhisperContextPort};

/// Orchestrates transcribe phase transitions, model readiness, and worker lifecycle.
pub trait TranscribeOrchestrator: Send {
    fn ensure_model(&mut self) -> Result<(), TranscribeError>;
    /// Transitions `idle`/`error` → `loading_model` without downloading.
    fn begin_model_loading(&mut self) -> Result<(), TranscribeError>;
    /// Loads whisper context from a verified local model path.
    fn finish_model_loading(&mut self, path: &Path) -> Result<(), TranscribeError>;
    /// Marks model acquisition failure (`loading_model` → `error`).
    fn fail_model_loading(&mut self);
    fn set_model_progress_callback(
        &mut self,
        callback: Box<dyn FnMut(ModelDownloadProgress) + Send>,
    );
    fn start(&mut self) -> Result<(), TranscribeError>;
    fn stop(&mut self) -> Result<(), TranscribeError>;
    fn pause_capture(&mut self);
    fn phase(&self) -> TranscribePhase;
    fn on_upstream_capture_error(&mut self);
    fn set_upstream_capturing(&mut self, capturing: bool);
    /// Marks inference failure during transcribing (`transcribing` → `error`).
    fn fail_inference(&mut self);
}

/// Default orchestrator: gates `start` on `ready` + upstream capturing, joins worker on `stop`.
pub struct DefaultTranscribeOrchestrator<W, C, S, D> {
    phase: TranscribePhase,
    worker: W,
    _context: PhantomData<C>,
    model_orchestrator: Arc<Mutex<ModelOrchestrator<S, D>>>,
    stop_timeout: Duration,
    upstream_capturing: bool,
    on_model_progress: Box<dyn FnMut(ModelDownloadProgress) + Send>,
}

impl<W: TranscribeWorkerPort, C, S, D> DefaultTranscribeOrchestrator<W, C, S, D> {
    pub fn new(
        worker: W,
        _context: C,
        model_orchestrator: Arc<Mutex<ModelOrchestrator<S, D>>>,
        stop_timeout: Duration,
    ) -> Self {
        Self {
            phase: TranscribePhase::Idle,
            worker,
            _context: PhantomData,
            model_orchestrator,
            stop_timeout,
            upstream_capturing: false,
            on_model_progress: Box::new(|_| {}),
        }
    }

    /// Tracks upstream audio-capture `capturing` state for start gating (req 6.1, 6.4).
    pub fn set_upstream_capturing(&mut self, capturing: bool) {
        self.upstream_capturing = capturing;
    }

    pub fn upstream_capturing(&self) -> bool {
        self.upstream_capturing
    }

    /// Registers a callback invoked during `ensure_model` download progress (req 5.1).
    pub fn set_model_progress_callback(
        &mut self,
        callback: Box<dyn FnMut(ModelDownloadProgress) + Send>,
    ) {
        self.on_model_progress = callback;
    }

    fn transition_to(&mut self, target: TranscribePhase) -> Result<(), TranscribeError> {
        self.phase =
            self.phase
                .try_transition_to(target)
                .map_err(|err| TranscribeError::Internal {
                    detail: err.to_string(),
                })?;
        Ok(())
    }

    fn stop_worker(&mut self) {
        let _ = self.worker.stop_and_join(self.stop_timeout);
    }
}

impl<W: TranscribeWorkerPort, C: WhisperContextPort, S, D> DefaultTranscribeOrchestrator<W, C, S, D>
where
    S: super::ports::ModelStorePort,
    D: super::ports::ModelDownloaderPort,
{
    fn ensure_model_loaded(&mut self, model_path: &Path) -> Result<(), TranscribeError> {
        // Defer whisper context creation to the worker thread (whisper.cpp is not thread-safe
        // across load/inference when the context is moved between threads).
        let result = self.worker.prepare_model_path(model_path);
        self.apply_worker_prepare_result(result)
    }

    fn apply_worker_prepare_result(
        &mut self,
        result: Result<(), TranscribeError>,
    ) -> Result<(), TranscribeError> {
        match result {
            Ok(()) => {
                self.transition_to(TranscribePhase::Ready)?;
                Ok(())
            }
            Err(err) => {
                self.phase = TranscribePhase::Error;
                Err(err)
            }
        }
    }

    fn halt_transcribing_on_upstream_stop(&mut self) {
        self.upstream_capturing = false;
        if self.phase != TranscribePhase::Transcribing {
            return;
        }
        self.stop_worker();
        self.phase = TranscribePhase::Ready;
    }

    pub fn ensure_model_inner(&mut self) -> Result<(), TranscribeError> {
        self.begin_model_loading()?;
        let acquire_result = {
            let model = self
                .model_orchestrator
                .lock()
                .map_err(|_| TranscribeError::Internal {
                    detail: "model orchestrator lock poisoned".to_string(),
                })?;
            model.ensure_model(&mut self.on_model_progress)
        };
        match acquire_result {
            Ok(path) => self.finish_model_loading(&path),
            Err(err) => {
                self.fail_model_loading();
                Err(err)
            }
        }
    }

    pub fn begin_model_loading_inner(&mut self) -> Result<(), TranscribeError> {
        if self.phase == TranscribePhase::Ready {
            return Ok(());
        }
        if self.phase == TranscribePhase::Transcribing || self.phase == TranscribePhase::Stopping {
            return Err(TranscribeError::Internal {
                detail: format!("cannot ensure model while {}", self.phase.as_str()),
            });
        }

        if self.phase == TranscribePhase::Idle || self.phase == TranscribePhase::Error {
            self.transition_to(TranscribePhase::LoadingModel)?;
        }
        Ok(())
    }

    pub fn finish_model_loading_inner(&mut self, model_path: &Path) -> Result<(), TranscribeError> {
        self.ensure_model_loaded(model_path)
    }
}

impl<W: TranscribeWorkerPort, C: WhisperContextPort, S, D> TranscribeOrchestrator
    for DefaultTranscribeOrchestrator<W, C, S, D>
where
    S: super::ports::ModelStorePort,
    D: super::ports::ModelDownloaderPort,
{
    fn phase(&self) -> TranscribePhase {
        self.phase
    }

    fn ensure_model(&mut self) -> Result<(), TranscribeError> {
        self.ensure_model_inner()
    }

    fn begin_model_loading(&mut self) -> Result<(), TranscribeError> {
        self.begin_model_loading_inner()
    }

    fn finish_model_loading(&mut self, path: &Path) -> Result<(), TranscribeError> {
        self.finish_model_loading_inner(path)
    }

    fn fail_model_loading(&mut self) {
        self.phase = TranscribePhase::Error;
    }

    fn set_model_progress_callback(
        &mut self,
        callback: Box<dyn FnMut(ModelDownloadProgress) + Send>,
    ) {
        self.on_model_progress = callback;
    }

    fn start(&mut self) -> Result<(), TranscribeError> {
        if self.phase != TranscribePhase::Ready {
            return Err(TranscribeError::Internal {
                detail: format!("cannot start from {}", self.phase.as_str()),
            });
        }
        if !self.upstream_capturing {
            return Err(TranscribeError::Internal {
                detail: "cannot start while upstream capture is not active".to_string(),
            });
        }

        self.transition_to(TranscribePhase::Transcribing)?;

        if let Err(err) = self.worker.spawn() {
            self.stop_worker();
            self.phase = TranscribePhase::Error;
            return Err(err);
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), TranscribeError> {
        match self.phase {
            TranscribePhase::Idle | TranscribePhase::Ready => return Ok(()),
            TranscribePhase::Stopping => return Ok(()),
            TranscribePhase::LoadingModel | TranscribePhase::Error => {
                self.stop_worker();
                self.phase = TranscribePhase::Idle;
                return Ok(());
            }
            TranscribePhase::Transcribing => {}
        }

        self.transition_to(TranscribePhase::Stopping)?;
        self.stop_worker();
        self.phase = TranscribePhase::Idle;
        Ok(())
    }

    fn pause_capture(&mut self) {
        self.halt_transcribing_on_upstream_stop();
    }

    fn on_upstream_capture_error(&mut self) {
        self.halt_transcribing_on_upstream_stop();
    }

    fn set_upstream_capturing(&mut self, capturing: bool) {
        self.upstream_capturing = capturing;
    }

    fn fail_inference(&mut self) {
        if self.phase == TranscribePhase::Transcribing {
            self.stop_worker();
        }
        self.upstream_capturing = false;
        self.phase = TranscribePhase::Error;
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use gijirec_domain::transcribe::TranscribeErrorCode;

    use super::*;
    use crate::transcribe::test_support::{
        NoopModelDownloader, QueueModelStore, default_model_path,
    };
    struct MockWorker {
        prepare_calls: AtomicUsize,
        spawn_calls: AtomicUsize,
        stop_calls: AtomicUsize,
        spawn_result: Mutex<Result<(), TranscribeError>>,
        last_prepared_path: Mutex<Option<PathBuf>>,
    }

    impl MockWorker {
        fn new() -> Arc<Mutex<Self>> {
            Arc::new(Mutex::new(Self {
                prepare_calls: AtomicUsize::new(0),
                spawn_calls: AtomicUsize::new(0),
                stop_calls: AtomicUsize::new(0),
                spawn_result: Mutex::new(Ok(())),
                last_prepared_path: Mutex::new(None),
            }))
        }

        fn prepare_call_count(worker: &Arc<Mutex<Self>>) -> usize {
            worker
                .lock()
                .expect("lock")
                .prepare_calls
                .load(Ordering::SeqCst)
        }

        fn prepared_path(worker: &Arc<Mutex<Self>>) -> Option<PathBuf> {
            worker
                .lock()
                .expect("lock")
                .last_prepared_path
                .lock()
                .expect("lock")
                .clone()
        }

        fn spawn_call_count(worker: &Arc<Mutex<Self>>) -> usize {
            worker
                .lock()
                .expect("lock")
                .spawn_calls
                .load(Ordering::SeqCst)
        }

        fn stop_call_count(worker: &Arc<Mutex<Self>>) -> usize {
            worker
                .lock()
                .expect("lock")
                .stop_calls
                .load(Ordering::SeqCst)
        }

        fn set_spawn_failure(worker: &Arc<Mutex<Self>>, err: TranscribeError) {
            *worker
                .lock()
                .expect("lock")
                .spawn_result
                .lock()
                .expect("lock") = Err(err);
        }
    }

    impl TranscribeWorkerPort for Arc<Mutex<MockWorker>> {
        fn prepare_model_path(&mut self, path: &Path) -> Result<(), TranscribeError> {
            let inner = self.lock().expect("lock");
            inner.prepare_calls.fetch_add(1, Ordering::SeqCst);
            *inner.last_prepared_path.lock().expect("lock") = Some(path.to_path_buf());
            Ok(())
        }

        fn spawn(&mut self) -> Result<(), TranscribeError> {
            let inner = self.lock().expect("lock");
            inner.spawn_calls.fetch_add(1, Ordering::SeqCst);
            inner.spawn_result.lock().expect("lock").clone()
        }

        fn stop_and_join(&mut self, _timeout: Duration) -> Result<(), TranscribeError> {
            self.lock()
                .expect("lock")
                .stop_calls
                .fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct MockContext {
        load_calls: AtomicUsize,
        load_result: Mutex<Result<(), TranscribeError>>,
        last_path: Mutex<Option<PathBuf>>,
    }

    impl MockContext {
        fn new() -> Arc<Mutex<Self>> {
            Arc::new(Mutex::new(Self {
                load_calls: AtomicUsize::new(0),
                load_result: Mutex::new(Ok(())),
                last_path: Mutex::new(None),
            }))
        }

        fn load_call_count(context: &Arc<Mutex<Self>>) -> usize {
            context
                .lock()
                .expect("lock")
                .load_calls
                .load(Ordering::SeqCst)
        }
    }

    impl WhisperContextPort for Arc<Mutex<MockContext>> {
        fn load_model(&mut self, path: &Path) -> Result<(), TranscribeError> {
            let inner = self.lock().expect("lock");
            inner.load_calls.fetch_add(1, Ordering::SeqCst);
            *inner.last_path.lock().expect("lock") = Some(path.to_path_buf());
            inner.load_result.lock().expect("lock").clone()
        }
    }

    type TestOrchestrator = DefaultTranscribeOrchestrator<
        Arc<Mutex<MockWorker>>,
        Arc<Mutex<MockContext>>,
        Arc<QueueModelStore>,
        NoopModelDownloader,
    >;

    fn orchestrator(
        worker: Arc<Mutex<MockWorker>>,
        context: Arc<Mutex<MockContext>>,
        store: Arc<QueueModelStore>,
    ) -> TestOrchestrator {
        let model_orchestrator = Arc::new(Mutex::new(ModelOrchestrator::new(
            store,
            NoopModelDownloader,
        )));
        DefaultTranscribeOrchestrator::new(
            worker,
            context,
            model_orchestrator,
            Duration::from_secs(5),
        )
    }

    fn fresh_orchestrator() -> (
        TestOrchestrator,
        Arc<Mutex<MockWorker>>,
        Arc<Mutex<MockContext>>,
    ) {
        let worker = MockWorker::new();
        let context = MockContext::new();
        let store = QueueModelStore::with_valid_model();
        let orch = orchestrator(Arc::clone(&worker), Arc::clone(&context), store);
        (orch, worker, context)
    }

    fn ready_orchestrator() -> (
        TestOrchestrator,
        Arc<Mutex<MockWorker>>,
        Arc<Mutex<MockContext>>,
    ) {
        let (mut orch, worker, context) = fresh_orchestrator();
        orch.ensure_model().expect("ensure model");
        assert_eq!(orch.phase(), TranscribePhase::Ready);
        (orch, worker, context)
    }

    #[test]
    fn ensure_model_transitions_to_ready_and_prepares_worker_model_path() {
        let (mut orch, worker, context) = fresh_orchestrator();

        assert_eq!(orch.phase(), TranscribePhase::Idle);
        orch.ensure_model().expect("ensure model");

        assert_eq!(orch.phase(), TranscribePhase::Ready);
        assert_eq!(MockWorker::prepare_call_count(&worker), 1);
        assert_eq!(
            MockWorker::prepared_path(&worker),
            Some(default_model_path())
        );
        assert_eq!(MockContext::load_call_count(&context), 0);
        assert_eq!(MockWorker::spawn_call_count(&worker), 0);
    }

    #[test]
    fn ensure_model_failure_transitions_to_error() {
        let worker = MockWorker::new();
        let context = MockContext::new();
        let store = QueueModelStore::without_verify_tracking(
            default_model_path(),
            vec![
                Err(TranscribeError::ModelNotFound {
                    detail: "missing".to_string(),
                }),
                Err(TranscribeError::ModelNotFound {
                    detail: "still missing after download".to_string(),
                }),
            ],
        );
        let model_orchestrator = Arc::new(Mutex::new(ModelOrchestrator::new(
            Arc::clone(&store),
            NoopModelDownloader,
        )));
        let mut orch = DefaultTranscribeOrchestrator::new(
            worker,
            context,
            model_orchestrator,
            Duration::from_secs(5),
        );

        let err = orch.ensure_model().expect_err("model missing");
        assert!(matches!(err, TranscribeError::ModelNotFound { .. }));
        assert_eq!(orch.phase(), TranscribePhase::Error);
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::ModelNotFound
        );
    }

    #[test]
    fn start_from_ready_with_upstream_capturing_starts_worker() {
        let (mut orch, worker, _) = ready_orchestrator();
        orch.set_upstream_capturing(true);

        orch.start().expect("start");

        assert_eq!(orch.phase(), TranscribePhase::Transcribing);
        assert_eq!(MockWorker::spawn_call_count(&worker), 1);
    }

    #[test]
    fn start_from_non_ready_fails_without_spawning_worker() {
        let (mut orch, worker, _context) = fresh_orchestrator();
        orch.set_upstream_capturing(true);

        let err = orch.start().expect_err("idle start");
        assert!(matches!(err, TranscribeError::Internal { .. }));
        assert_eq!(orch.phase(), TranscribePhase::Idle);
        assert_eq!(MockWorker::spawn_call_count(&worker), 0);
    }

    #[test]
    fn start_without_upstream_capturing_fails_and_does_not_spawn_worker() {
        let (mut orch, worker, _) = ready_orchestrator();

        let err = orch.start().expect_err("no upstream capture");
        assert!(matches!(err, TranscribeError::Internal { .. }));
        assert_eq!(orch.phase(), TranscribePhase::Ready);
        assert_eq!(MockWorker::spawn_call_count(&worker), 0);
    }

    #[test]
    fn stop_from_transcribing_joins_worker_and_returns_idle() {
        let (mut orch, worker, _) = ready_orchestrator();
        orch.set_upstream_capturing(true);
        orch.start().expect("start");

        orch.stop().expect("stop");

        assert_eq!(orch.phase(), TranscribePhase::Idle);
        assert_eq!(MockWorker::stop_call_count(&worker), 1);
    }

    #[test]
    fn on_upstream_capture_error_stops_worker_and_returns_ready() {
        let (mut orch, worker, _) = ready_orchestrator();
        orch.set_upstream_capturing(true);
        orch.start().expect("start");

        orch.on_upstream_capture_error();

        assert_eq!(orch.phase(), TranscribePhase::Ready);
        assert!(!orch.upstream_capturing());
        assert_eq!(MockWorker::stop_call_count(&worker), 1);
    }

    #[test]
    fn worker_spawn_failure_transitions_to_error() {
        let (mut orch, worker, _) = ready_orchestrator();
        orch.set_upstream_capturing(true);
        MockWorker::set_spawn_failure(
            &worker,
            TranscribeError::InferenceFailed {
                detail: "spawn failed".to_string(),
            },
        );

        let err = orch.start().expect_err("spawn failure");
        assert!(matches!(err, TranscribeError::InferenceFailed { .. }));
        assert_eq!(orch.phase(), TranscribePhase::Error);
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::InferenceFailed
        );
        assert_eq!(MockWorker::spawn_call_count(&worker), 1);
        assert_eq!(MockWorker::stop_call_count(&worker), 1);
    }
}
