//! Dedicated-thread PCM consumer and fixed-interval batch whisper inference worker.

mod batch_cycle;
mod batch_window;
mod deps;
mod engine;
mod pcm_buffer;
mod types;
mod worker_loop;

#[cfg(test)]
mod tests;

use deps::{
    Arc, AtomicBool, AtomicU64, Duration, Instant, JoinHandle, Ordering, TranscribeError,
    TranscriptSegmentSink, thread,
};

use crate::transcribe::whisper_adapter::WhisperCppAdapter;

pub use engine::{ModelPathLoadable, SegmentEngine};
pub use pcm_buffer::MAX_PCM_BUFFER_SAMPLES;
pub use types::{BatchCycleCompleted, BatchCycleStarted, InferenceWindowLevel};

use types::{BatchCycleCompletedCallback, BatchCycleStartedCallback, InferenceWindowLevelCallback};
use worker_loop::{WorkerHooks, WorkerParams, worker_loop};

/// Fixed-interval batch inference worker reading PCM from an rtrb consumer.
pub struct TranscribeWorker<E: SegmentEngine + 'static = WhisperCppAdapter> {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pcm_consumer: Option<rtrb::Consumer<f32>>,
    engine: Option<E>,
    pending_model_path: Option<std::path::PathBuf>,
    sink: Arc<dyn TranscriptSegmentSink>,
    hooks: WorkerHooks,
    on_inference_progress: Option<Arc<dyn Fn(i32) + Send + Sync>>,
}

impl<E: SegmentEngine + 'static> TranscribeWorker<E> {
    pub fn prepare_model_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        Self::prepare_model_path_inner(self, path)
    }

    pub fn new(sink: Arc<dyn TranscriptSegmentSink>) -> Self
    where
        E: Default,
    {
        Self::with_engine(sink, E::default())
    }

    pub fn with_engine(sink: Arc<dyn TranscriptSegmentSink>, engine: E) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            pcm_consumer: None,
            engine: Some(engine),
            pending_model_path: None,
            sink,
            hooks: WorkerHooks::empty(),
            on_inference_progress: None,
        }
    }

    pub fn install_engine(&mut self, engine: E) {
        self.engine = Some(engine);
    }

    pub fn is_engine_loaded(&self) -> bool {
        self.engine.as_ref().is_some_and(SegmentEngine::is_loaded)
    }

    fn prepare_model_path_inner(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        if !path.is_file() {
            return Err(TranscribeError::ModelNotFound {
                detail: format!("model path is not a file: {}", path.display()),
            });
        }
        self.pending_model_path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn attach_pcm_consumer(&mut self, consumer: rtrb::Consumer<f32>) {
        self.pcm_consumer = Some(consumer);
    }

    pub fn set_inference_latency_callback(&mut self, callback: Arc<dyn Fn(u64) + Send + Sync>) {
        self.hooks.on_inference_latency_ms = Some(callback);
    }

    pub fn set_fatal_error_callback(
        &mut self,
        callback: Arc<dyn Fn(TranscribeError) + Send + Sync>,
    ) {
        self.hooks.on_fatal = Some(callback);
    }

    pub fn set_engine_ready_callback(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.hooks.on_engine_ready = Some(callback);
    }

    pub fn set_inference_attempted_callback(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.hooks.on_inference_attempted = Some(callback);
    }

    pub fn set_inference_progress_callback(&mut self, callback: Arc<dyn Fn(i32) + Send + Sync>) {
        self.on_inference_progress = Some(callback);
    }

    pub fn set_batch_cycle_started_callback(&mut self, callback: BatchCycleStartedCallback) {
        self.hooks.on_batch_cycle_started = Some(callback);
    }

    pub fn set_batch_cycle_completed_callback(&mut self, callback: BatchCycleCompletedCallback) {
        self.hooks.on_batch_cycle_completed = Some(callback);
    }

    pub fn set_inference_window_level_callback(&mut self, callback: InferenceWindowLevelCallback) {
        self.hooks.on_inference_window_level = Some(callback);
    }

    /// Optional shared counter incremented when rtrb ingest hits backpressure.
    pub fn set_rtrb_overflow_counter(&mut self, counter: Arc<AtomicU64>) {
        self.hooks.rtrb_overflow_count = Some(counter);
    }

    pub fn is_active(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn spawn(&mut self) -> Result<(), TranscribeError>
    where
        E: ModelPathLoadable,
    {
        if self.handle.is_some() {
            return Err(TranscribeError::Internal {
                detail: "transcribe worker already running".to_string(),
            });
        }

        let consumer = self
            .pcm_consumer
            .take()
            .ok_or_else(|| TranscribeError::Internal {
                detail: "pcm consumer not attached".to_string(),
            })?;
        let mut engine = self
            .engine
            .take()
            .ok_or_else(|| TranscribeError::Internal {
                detail: "inference engine not available".to_string(),
            })?;
        if let Some(hook) = self.on_inference_progress.clone() {
            engine.set_progress_hook(hook);
        }
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let model_path = self.pending_model_path.clone();
        let sink = Arc::clone(&self.sink);
        let hooks = self.hooks.clone();

        let params = WorkerParams {
            consumer,
            engine,
            model_path,
            sink,
            running,
            hooks,
        };
        let handle = thread::Builder::new()
            .name("transcribe-worker".into())
            .spawn(move || {
                worker_loop(params);
            })
            .map_err(|err| TranscribeError::InferenceFailed {
                detail: format!("failed to spawn transcribe worker: {err}"),
            })?;

        self.handle = Some(handle);
        Ok(())
    }

    pub fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError> {
        self.running.store(false, Ordering::SeqCst);

        let Some(handle) = self.handle.take() else {
            self.engine = None;
            return Ok(());
        };

        let deadline = Instant::now() + timeout;
        loop {
            if handle.is_finished() {
                let _ = handle.join();
                break;
            }
            if Instant::now() >= deadline {
                eprintln!(
                    "WARN: transcribe worker join timed out after {timeout:?}, detaching handle"
                );
                drop(handle);
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        self.engine = None;
        Ok(())
    }
}
