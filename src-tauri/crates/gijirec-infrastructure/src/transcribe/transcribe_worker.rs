//! Dedicated-thread PCM consumer and fixed-interval batch whisper inference worker.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use gijirec_domain::transcribe::{TranscribeError, TranscriptSegmentSink};

use super::whisper_adapter::{WhisperCppAdapter, WhisperSegment};

/// Samples per endpointing frame (100 ms @ 16 kHz, same length as an upstream PCM chunk).
const FRAME_SAMPLES: usize = 1_600;

/// Longest PCM handed to whisper.cpp in one batch inference (30 s @ 16 kHz).
const MAX_INFERENCE_WINDOW_SAMPLES: usize = 480_000;

/// Maximum samples retained while waiting for the next inference window (10 min @ 16 kHz).
/// Large enough to hold backlog when inference falls behind without dropping audio.
pub const MAX_PCM_BUFFER_SAMPLES: usize = 9_600_000;

/// Speech required before a short trailing pause closes the utterance (1 s @ 16 kHz).
const MIN_SPEECH_SAMPLES: usize = 16_000;

/// Trailing near-silence that closes an utterance once enough speech accumulated (1.2 s).
const TRAILING_SILENCE_FRAMES: usize = 12;

/// Longer near-silence that closes an utterance regardless of its length (4.8 s).
const LONG_SILENCE_FRAMES: usize = 48;

/// When forced to cut at the max window, search the last 2 s for the quietest frame.
const FORCED_CUT_SEARCH_SAMPLES: usize = 32_000;

/// Smallest unfinished tail still transcribed when the worker shuts down (500 ms).
const MIN_FLUSH_SAMPLES: usize = 8_000;

const SAMPLE_RATE_HZ: u64 = 16_000;

/// Fixed delay between completed batch inference cycles (30 s in production).
#[cfg(not(test))]
const BATCH_INTERVAL: Duration = Duration::from_secs(30);

/// Shorter interval for unit/integration tests that exercise batch timing.
#[cfg(test)]
const BATCH_INTERVAL: Duration = Duration::from_millis(100);

/// Skip whisper.cpp when the window RMS is below this (near-silence).
const SILENCE_RMS_THRESHOLD: f32 = 0.008;

/// Structured fields for a batch inference cycle start event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchCycleStarted {
    pub cycle_id: u64,
    pub samples_count: usize,
    pub pcm_backlog_seconds: f64,
    pub rtrb_overflow_count: u64,
}

/// Structured fields for a batch inference cycle completion event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchCycleCompleted {
    pub cycle_id: u64,
    pub duration_ms: u64,
    pub samples_count: usize,
    pub segments_count: usize,
}

type BatchCycleStartedCallback = Arc<dyn Fn(BatchCycleStarted) -> Option<std::path::PathBuf> + Send + Sync>;
type BatchCycleCompletedCallback = Arc<dyn Fn(BatchCycleCompleted) + Send + Sync>;

/// PCM level metrics for one whisper.cpp inference window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InferenceWindowLevel {
    pub window_rms: f32,
    pub samples_count: usize,
    pub inference_skipped: bool,
}

pub(crate) type InferenceWindowLevelCallback = Arc<dyn Fn(InferenceWindowLevel) + Send + Sync>;

/// Inference engine abstraction (production: [`WhisperCppAdapter`], tests: mocks).
pub trait SegmentEngine: Send {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError>;
    fn is_loaded(&self) -> bool;
    fn set_progress_hook(&mut self, _hook: Arc<dyn Fn(i32) + Send + Sync>) {}
    fn set_running_flag(&mut self, _running: Arc<AtomicBool>) {}
}

/// Loads a whisper model path on the worker thread before inference begins.
pub trait ModelPathLoadable: SegmentEngine {
    fn load_from_path_if_needed(&mut self, path: &std::path::Path) -> Result<(), TranscribeError>;
    fn reload_from_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        self.load_from_path_if_needed(path)
    }
}

impl ModelPathLoadable for WhisperCppAdapter {
    fn load_from_path_if_needed(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        if self.is_loaded() {
            return Ok(());
        }
        self.load_model(path)
    }

    fn reload_from_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        self.reload_model(path)
    }
}

impl SegmentEngine for WhisperCppAdapter {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        WhisperCppAdapter::transcribe_pcm(self, pcm)
    }

    fn is_loaded(&self) -> bool {
        WhisperCppAdapter::is_loaded(self)
    }

    fn set_progress_hook(&mut self, hook: Arc<dyn Fn(i32) + Send + Sync>) {
        WhisperCppAdapter::set_progress_hook(self, hook);
    }
}

/// Fixed-interval batch inference worker reading PCM from an rtrb consumer.
pub struct TranscribeWorker<E: SegmentEngine + 'static = WhisperCppAdapter> {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pcm_consumer: Option<rtrb::Consumer<f32>>,
    engine: Option<E>,
    pending_model_path: Option<std::path::PathBuf>,
    sink: Arc<dyn TranscriptSegmentSink>,
    on_inference_latency_ms: Option<Arc<dyn Fn(u64) + Send + Sync>>,
    on_fatal: Option<Arc<dyn Fn(TranscribeError) + Send + Sync>>,
    on_engine_ready: Option<Arc<dyn Fn() + Send + Sync>>,
    on_inference_attempted: Option<Arc<dyn Fn() + Send + Sync>>,
    on_inference_progress: Option<Arc<dyn Fn(i32) + Send + Sync>>,
    on_batch_cycle_started: Option<BatchCycleStartedCallback>,
    on_batch_cycle_completed: Option<BatchCycleCompletedCallback>,
    on_inference_window_level: Option<InferenceWindowLevelCallback>,
    rtrb_overflow_count: Option<Arc<AtomicU64>>,
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
            on_inference_latency_ms: None,
            on_fatal: None,
            on_engine_ready: None,
            on_inference_attempted: None,
            on_inference_progress: None,
            on_batch_cycle_started: None,
            on_batch_cycle_completed: None,
            on_inference_window_level: None,
            rtrb_overflow_count: None,
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
        self.on_inference_latency_ms = Some(callback);
    }

    pub fn set_fatal_error_callback(
        &mut self,
        callback: Arc<dyn Fn(TranscribeError) + Send + Sync>,
    ) {
        self.on_fatal = Some(callback);
    }

    pub fn set_engine_ready_callback(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.on_engine_ready = Some(callback);
    }

    pub fn set_inference_attempted_callback(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.on_inference_attempted = Some(callback);
    }

    pub fn set_inference_progress_callback(&mut self, callback: Arc<dyn Fn(i32) + Send + Sync>) {
        self.on_inference_progress = Some(callback);
    }

    pub fn set_batch_cycle_started_callback(&mut self, callback: BatchCycleStartedCallback) {
        self.on_batch_cycle_started = Some(callback);
    }

    pub fn set_batch_cycle_completed_callback(&mut self, callback: BatchCycleCompletedCallback) {
        self.on_batch_cycle_completed = Some(callback);
    }

    pub fn set_inference_window_level_callback(&mut self, callback: InferenceWindowLevelCallback) {
        self.on_inference_window_level = Some(callback);
    }

    /// Optional shared counter incremented when rtrb ingest hits backpressure.
    pub fn set_rtrb_overflow_counter(&mut self, counter: Arc<AtomicU64>) {
        self.rtrb_overflow_count = Some(counter);
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
        let on_latency = self.on_inference_latency_ms.clone();
        let on_fatal = self.on_fatal.clone();
        let on_engine_ready = self.on_engine_ready.clone();
        let on_inference_attempted = self.on_inference_attempted.clone();
        let on_batch_cycle_started = self.on_batch_cycle_started.clone();
        let on_batch_cycle_completed = self.on_batch_cycle_completed.clone();
        let on_inference_window_level = self.on_inference_window_level.clone();
        let rtrb_overflow_count = self.rtrb_overflow_count.clone();

        let params = WorkerParams {
            consumer,
            engine,
            model_path,
            sink,
            running,
            on_latency,
            on_fatal,
            on_engine_ready,
            on_inference_attempted,
            on_batch_cycle_started,
            on_batch_cycle_completed,
            on_inference_window_level,
            rtrb_overflow_count,
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

struct WorkerParams<E> {
    consumer: rtrb::Consumer<f32>,
    engine: E,
    model_path: Option<std::path::PathBuf>,
    sink: Arc<dyn TranscriptSegmentSink>,
    running: Arc<AtomicBool>,
    on_latency: Option<Arc<dyn Fn(u64) + Send + Sync>>,
    on_fatal: Option<Arc<dyn Fn(TranscribeError) + Send + Sync>>,
    on_engine_ready: Option<Arc<dyn Fn() + Send + Sync>>,
    on_inference_attempted: Option<Arc<dyn Fn() + Send + Sync>>,
    on_batch_cycle_started: Option<BatchCycleStartedCallback>,
    on_batch_cycle_completed: Option<BatchCycleCompletedCallback>,
    on_inference_window_level: Option<InferenceWindowLevelCallback>,
    rtrb_overflow_count: Option<Arc<AtomicU64>>,
}

struct PcmBufferState {
    samples: VecDeque<f32>,
    samples_before_buffer: u64,
}

struct InferenceContext<'a, E> {
    engine: &'a mut E,
    sink: &'a Arc<dyn TranscriptSegmentSink>,
    on_latency: Option<&'a Arc<dyn Fn(u64) + Send + Sync>>,
    on_attempted: Option<&'a Arc<dyn Fn() + Send + Sync>>,
    on_window_level: Option<&'a InferenceWindowLevelCallback>,
}

struct InferenceOutcome {
    segments_count: usize,
}

fn worker_loop<E: ModelPathLoadable>(params: WorkerParams<E>) {
    let WorkerParams {
        consumer,
        mut engine,
        model_path,
        sink,
        running,
        on_latency,
        on_fatal,
        on_engine_ready,
        on_inference_attempted,
        on_batch_cycle_started,
        on_batch_cycle_completed,
        on_inference_window_level,
        rtrb_overflow_count,
    } = params;

    let pcm_buffer = Arc::new(Mutex::new(PcmBufferState {
        samples: VecDeque::new(),
        samples_before_buffer: 0,
    }));
    let drain_buffer = Arc::clone(&pcm_buffer);
    let drain_running = Arc::clone(&running);
    let drain_handle = thread::Builder::new()
        .name("transcribe-pcm-drain".into())
        .spawn(move || drain_pcm_loop(consumer, drain_buffer, drain_running))
        .map_err(|err| eprintln!("failed to spawn pcm drain thread: {err}"))
        .ok();

    let load_result = if let Some(path) = model_path {
        engine.load_from_path_if_needed(&path)
    } else if engine.is_loaded() {
        Ok(())
    } else {
        Err(TranscribeError::Internal {
            detail: "transcribe worker started without model path and engine is not loaded"
                .to_string(),
        })
    };

    if let Err(err) = load_result {
        running.store(false, Ordering::SeqCst);
        if let Some(handle) = drain_handle {
            let _ = handle.join();
        }
        if let Some(notify) = on_fatal {
            thread::spawn(move || notify(err));
        }
        return;
    }

    if !running.load(Ordering::SeqCst) {
        if let Some(handle) = drain_handle {
            let _ = handle.join();
        }
        return;
    }

    if let Some(notify) = on_engine_ready {
        notify();
    }

    let transcribing_start = Instant::now();
    let mut last_cycle_complete: Option<Instant> = None;
    let mut backlog_after_last_cycle = false;
    let mut cycle_id = 0u64;

    while running.load(Ordering::SeqCst) {
        let unprocessed = {
            let state = pcm_buffer.lock().expect("pcm buffer lock");
            state.samples.len()
        };

        let ready = match last_cycle_complete {
            None => first_cycle_ready(transcribing_start, unprocessed),
            Some(completed_at) => {
                next_cycle_ready(completed_at, unprocessed, backlog_after_last_cycle)
            }
        };

        if ready {
            if let Some((pcm, base_samples)) = take_batch_window(&pcm_buffer) {
                cycle_id = cycle_id.saturating_add(1);
                run_batch_cycle(
                    cycle_id,
                    pcm,
                    base_samples,
                    samples_to_seconds(unprocessed),
                    read_rtrb_overflow_count(&rtrb_overflow_count),
                    engine.is_loaded(),
                    &mut engine,
                    &sink,
                    on_latency.as_ref(),
                    on_inference_attempted.as_ref(),
                    on_batch_cycle_started.as_ref(),
                    on_batch_cycle_completed.as_ref(),
                    on_inference_window_level.as_ref(),
                );
                last_cycle_complete = Some(Instant::now());
                backlog_after_last_cycle = {
                    let state = pcm_buffer.lock().expect("pcm buffer lock");
                    !state.samples.is_empty()
                };
            }
        } else {
            thread::sleep(Duration::from_millis(5));
        }
    }

    if let Some(handle) = drain_handle {
        let _ = handle.join();
    }

    while let Some((pcm, base_samples)) = take_batch_window(&pcm_buffer) {
        cycle_id = cycle_id.saturating_add(1);
        let flush_backlog_samples = {
            let state = pcm_buffer.lock().expect("pcm buffer lock");
            state.samples.len()
        };
        run_batch_cycle(
            cycle_id,
            pcm,
            base_samples,
            samples_to_seconds(flush_backlog_samples),
            read_rtrb_overflow_count(&rtrb_overflow_count),
            engine.is_loaded(),
            &mut engine,
            &sink,
            on_latency.as_ref(),
            on_inference_attempted.as_ref(),
            on_batch_cycle_started.as_ref(),
            on_batch_cycle_completed.as_ref(),
            on_inference_window_level.as_ref(),
        );
    }

    drop(engine);
}

fn drain_pcm_loop(
    mut consumer: rtrb::Consumer<f32>,
    pcm_buffer: Arc<Mutex<PcmBufferState>>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        let popped = {
            let mut state = pcm_buffer.lock().expect("pcm buffer lock");
            drain_consumer(&mut consumer, &mut state)
        };
        if popped == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }

    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    drain_consumer(&mut consumer, &mut state);
}

fn take_batch_window(pcm_buffer: &Arc<Mutex<PcmBufferState>>) -> Option<(Vec<f32>, u64)> {
    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    take_batch_window_from_state(&mut state)
}

/// Returns whether enough PCM has accumulated for a full 30 s inference window.
fn full_batch_window_ready(unprocessed_samples: usize) -> bool {
    unprocessed_samples >= MAX_INFERENCE_WINDOW_SAMPLES
}

/// Returns whether the first batch cycle should start (transcribing just began).
fn first_cycle_ready(_transcribing_start: Instant, unprocessed_samples: usize) -> bool {
    full_batch_window_ready(unprocessed_samples)
}

/// Returns whether a subsequent batch cycle should start after the previous one completed.
///
/// Inference requires a full 30 s window. When the previous cycle left another full window
/// in the backlog (`backlog_after_last_cycle`), the 30 s interval is skipped so catch-up
/// cycles run back-to-back. Otherwise the worker waits for another full window and the
/// batch interval since the previous cycle completed.
fn next_cycle_ready(
    last_cycle_complete: Instant,
    unprocessed_samples: usize,
    backlog_after_last_cycle: bool,
) -> bool {
    if !full_batch_window_ready(unprocessed_samples) {
        return false;
    }
    backlog_after_last_cycle || last_cycle_complete.elapsed() >= BATCH_INTERVAL
}

/// Cuts the next utterance off the buffer, or `None` when more audio is needed.
///
/// Endpointing runs on 100 ms frames: leading near-silence is discarded, the window
/// ends at the first pause long enough to mark an utterance boundary, and a window
/// that reaches [`MAX_INFERENCE_WINDOW_SAMPLES`] is cut at the quietest frame of its
/// last 2 s. With `flush`, a short unfinished tail is returned as well (shutdown).
/// Legacy VAD window cutting — retained for unit tests; production uses [`take_batch_window`].
#[allow(dead_code)]
fn take_inference_window(
    pcm_buffer: &Arc<Mutex<PcmBufferState>>,
    flush: bool,
) -> Option<(Vec<f32>, u64)> {
    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    take_window_from_state(&mut state, flush)
}

fn take_window_from_state(state: &mut PcmBufferState, flush: bool) -> Option<(Vec<f32>, u64)> {
    trim_leading_silence(state);

    let cut = {
        let buf = state.samples.make_contiguous();
        if let Some(end) = find_utterance_end(buf) {
            Some(end)
        } else if buf.len() >= MAX_INFERENCE_WINDOW_SAMPLES {
            Some(forced_cut_point(buf))
        } else if flush && buf.len() >= MIN_FLUSH_SAMPLES {
            Some(buf.len())
        } else {
            None
        }
    }?;

    let base_samples = state.samples_before_buffer;
    let pcm: Vec<f32> = state.samples.drain(..cut).collect();
    state.samples_before_buffer += cut as u64;
    Some((pcm, base_samples))
}

fn is_silent_frame(frame: &[f32]) -> bool {
    window_rms(frame) < SILENCE_RMS_THRESHOLD
}

/// Drops whole near-silent frames from the head so windows start on speech.
fn trim_leading_silence(state: &mut PcmBufferState) {
    let silent_frames = state
        .samples
        .make_contiguous()
        .as_chunks::<FRAME_SAMPLES>()
        .0
        .iter()
        .take_while(|frame| is_silent_frame(frame.as_slice()))
        .count();
    let drop = silent_frames * FRAME_SAMPLES;
    if drop > 0 {
        let _ = state.samples.drain(..drop);
        state.samples_before_buffer += drop as u64;
    }
}

/// Returns the sample index just after a pause that closes the current utterance.
///
/// A pause of [`TRAILING_SILENCE_FRAMES`] closes the utterance once at least
/// [`MIN_SPEECH_SAMPLES`] precede it; a pause of [`LONG_SILENCE_FRAMES`] closes it
/// regardless, so short replies are not held back waiting for more speech.
fn find_utterance_end(buf: &[f32]) -> Option<usize> {
    let mut silence_run = 0usize;
    for (index, frame) in buf.as_chunks::<FRAME_SAMPLES>().0.iter().enumerate() {
        let end = (index + 1) * FRAME_SAMPLES;
        if end > MAX_INFERENCE_WINDOW_SAMPLES {
            break;
        }
        if !is_silent_frame(frame.as_slice()) {
            silence_run = 0;
            continue;
        }
        silence_run += 1;
        let speech_len = end - silence_run * FRAME_SAMPLES;
        if silence_run >= LONG_SILENCE_FRAMES
            || (silence_run >= TRAILING_SILENCE_FRAMES && speech_len >= MIN_SPEECH_SAMPLES)
        {
            return Some(end);
        }
    }
    None
}

/// Picks the end of the quietest frame in the last 2 s of a full window so a forced
/// cut lands between words rather than inside one. Ties resolve to the latest frame.
fn forced_cut_point(buf: &[f32]) -> usize {
    debug_assert!(buf.len() >= MAX_INFERENCE_WINDOW_SAMPLES);
    let mut best_end = MAX_INFERENCE_WINDOW_SAMPLES;
    let mut best_rms = f32::INFINITY;
    let mut start = MAX_INFERENCE_WINDOW_SAMPLES - FORCED_CUT_SEARCH_SAMPLES;
    while start + FRAME_SAMPLES <= MAX_INFERENCE_WINDOW_SAMPLES {
        let end = start + FRAME_SAMPLES;
        let rms = window_rms(&buf[start..end]);
        if rms <= best_rms {
            best_rms = rms;
            best_end = end;
        }
        start = end;
    }
    best_end.max(MIN_SPEECH_SAMPLES)
}

fn drain_consumer(consumer: &mut rtrb::Consumer<f32>, state: &mut PcmBufferState) -> usize {
    let mut popped = 0usize;
    while let Ok(sample) = consumer.pop() {
        state.samples.push_back(sample);
        popped += 1;
    }
    popped
}

/// Cuts the next batch window from the buffer: up to [`MAX_INFERENCE_WINDOW_SAMPLES`]
/// from the front, with no VAD or leading-silence trimming.
fn take_batch_window_from_state(state: &mut PcmBufferState) -> Option<(Vec<f32>, u64)> {
    if state.samples.is_empty() {
        return None;
    }
    let cut = state.samples.len().min(MAX_INFERENCE_WINDOW_SAMPLES);
    let base_samples = state.samples_before_buffer;
    let pcm: Vec<f32> = state.samples.drain(..cut).collect();
    state.samples_before_buffer += cut as u64;
    Some((pcm, base_samples))
}

fn window_rms(pcm: &[f32]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = pcm.iter().map(|sample| sample * sample).sum();
    (sum_sq / pcm.len() as f32).sqrt()
}

fn run_batch_cycle<E: SegmentEngine + ModelPathLoadable>(
    cycle_id: u64,
    pcm: Vec<f32>,
    base_samples: u64,
    pcm_backlog_seconds: f64,
    rtrb_overflow_count: u64,
    engine_loaded: bool,
    engine: &mut E,
    sink: &Arc<dyn TranscriptSegmentSink>,
    on_latency: Option<&Arc<dyn Fn(u64) + Send + Sync>>,
    on_inference_attempted: Option<&Arc<dyn Fn() + Send + Sync>>,
    on_batch_cycle_started: Option<&BatchCycleStartedCallback>,
    on_batch_cycle_completed: Option<&BatchCycleCompletedCallback>,
    on_inference_window_level: Option<&InferenceWindowLevelCallback>,
) {
    let samples_count = pcm.len();
    let reload_path = if let Some(record) = on_batch_cycle_started {
        record(BatchCycleStarted {
            cycle_id,
            samples_count,
            pcm_backlog_seconds,
            rtrb_overflow_count,
        })
    } else {
        None
    };

    if let Some(path) = reload_path {
        if let Err(err) = engine.reload_from_path(&path) {
            eprintln!("WARN: batch cycle model reload failed, continuing with prior model: {err}");
        }
    }

    let cycle_start = Instant::now();
    let segments_count = if engine_loaded {
        let ctx = InferenceContext {
            engine,
            sink,
            on_latency,
            on_attempted: on_inference_attempted,
            on_window_level: on_inference_window_level,
        };
        run_inference_window(&pcm, base_samples, ctx).segments_count
    } else {
        0
    };

    if let Some(record) = on_batch_cycle_completed {
        record(BatchCycleCompleted {
            cycle_id,
            duration_ms: cycle_start.elapsed().as_millis() as u64,
            samples_count,
            segments_count,
        });
    }
}

fn run_inference_window<E: SegmentEngine>(
    pcm: &[f32],
    samples_before_buffer: u64,
    ctx: InferenceContext<'_, E>,
) -> InferenceOutcome {
    if pcm.is_empty() {
        return InferenceOutcome { segments_count: 0 };
    }

    let window_rms = window_rms(pcm);
    let inference_skipped = window_rms < SILENCE_RMS_THRESHOLD;
    if let Some(record) = ctx.on_window_level {
        record(InferenceWindowLevel {
            window_rms,
            samples_count: pcm.len(),
            inference_skipped,
        });
    }

    if inference_skipped {
        return InferenceOutcome { segments_count: 0 };
    }

    if let Some(attempted) = ctx.on_attempted {
        attempted();
    }

    let inference_start = Instant::now();
    let base_ms = samples_to_ms(samples_before_buffer);

    match ctx.engine.transcribe_pcm(pcm) {
        Ok(segments) => {
            let mut segments_count = 0usize;
            for segment in segments {
                let trimmed = segment.text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                segments_count += 1;
                let start_ms = base_ms.saturating_add(segment.start_ms.max(0) as u64);
                let _ = ctx.sink.on_segment(trimmed, start_ms, "auto");
            }
            if let Some(record) = ctx.on_latency {
                record(inference_start.elapsed().as_millis() as u64);
            }
            InferenceOutcome { segments_count }
        }
        Err(err) => {
            eprintln!("WARN: batch inference failed, continuing next cycle: {err}");
            if let Some(record) = ctx.on_latency {
                record(inference_start.elapsed().as_millis() as u64);
            }
            InferenceOutcome { segments_count: 0 }
        }
    }
}

fn read_rtrb_overflow_count(counter: &Option<Arc<AtomicU64>>) -> u64 {
    counter
        .as_ref()
        .map(|value| value.load(Ordering::Relaxed))
        .unwrap_or(0)
}

fn samples_to_seconds(samples: usize) -> f64 {
    samples as f64 / SAMPLE_RATE_HZ as f64
}

fn samples_to_ms(samples: u64) -> u64 {
    samples.saturating_mul(1_000) / SAMPLE_RATE_HZ
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::transcribe::TranscribeErrorCode;
    use rtrb::RingBuffer;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, AtomicUsize};

    struct RecordingSink {
        segments: Arc<Mutex<Vec<(String, u64, String)>>>,
    }

    impl TranscriptSegmentSink for RecordingSink {
        fn on_segment(
            &self,
            text: &str,
            start_ms: u64,
            language: &str,
        ) -> Result<(), TranscribeError> {
            self.segments.lock().expect("lock").push((
                text.to_string(),
                start_ms,
                language.to_string(),
            ));
            Ok(())
        }
    }

    struct MockEngine {
        segments: Vec<WhisperSegment>,
        loaded: bool,
        loaded_path: Option<std::path::PathBuf>,
        inference_started: Option<Arc<AtomicBool>>,
        block_until: Option<Arc<AtomicBool>>,
    }

    impl Default for MockEngine {
        fn default() -> Self {
            Self {
                segments: Vec::new(),
                loaded: true,
                loaded_path: None,
                inference_started: None,
                block_until: None,
            }
        }
    }

    fn wait_for_unblock(unblock: &Arc<AtomicBool>) {
        while !unblock.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(5));
        }
    }

    impl SegmentEngine for MockEngine {
        fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
            if let Some(flag) = &self.inference_started {
                flag.store(true, Ordering::SeqCst);
            }
            if let Some(unblock) = &self.block_until {
                wait_for_unblock(unblock);
            }
            if pcm.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self.segments.clone())
        }

        fn is_loaded(&self) -> bool {
            self.loaded
        }
    }

        impl ModelPathLoadable for MockEngine {
        fn load_from_path_if_needed(
            &mut self,
            path: &std::path::Path,
        ) -> Result<(), TranscribeError> {
            self.loaded_path = Some(path.to_path_buf());
            Ok(())
        }

        fn reload_from_path(
            &mut self,
            path: &std::path::Path,
        ) -> Result<(), TranscribeError> {
            self.loaded_path = Some(path.to_path_buf());
            Ok(())
        }
    }

    fn ring_pair(capacity: usize) -> (rtrb::Producer<f32>, rtrb::Consumer<f32>) {
        RingBuffer::<f32>::new(capacity)
    }

    /// Speech long enough for the short-pause rule, followed by a closing pause.
    const UTTERANCE_SPEECH_SAMPLES: usize = MIN_SPEECH_SAMPLES;
    const UTTERANCE_SILENCE_SAMPLES: usize = FRAME_SAMPLES * (TRAILING_SILENCE_FRAMES + 1);
    const UTTERANCE_SAMPLES: usize = UTTERANCE_SPEECH_SAMPLES + UTTERANCE_SILENCE_SAMPLES;
    /// Twice the trailing threshold so endpointing tests leave excess silence in the buffer.
    const TEST_PAUSE_PADDING_SAMPLES: usize = TRAILING_SILENCE_FRAMES * FRAME_SAMPLES * 2;

    fn push_samples(prod: &mut rtrb::Producer<f32>, value: f32, count: usize) {
        for _ in 0..count {
            prod.push(value).expect("push pcm");
        }
    }

    /// Pushes one complete utterance (speech then silence) so endpointing closes it.
    fn push_utterance(prod: &mut rtrb::Producer<f32>, value: f32) {
        push_samples(prod, value, UTTERANCE_SPEECH_SAMPLES);
        push_samples(prod, 0.0, UTTERANCE_SILENCE_SAMPLES);
    }

    /// Pads the producer to a full 30 s batch window (required before normal inference).
    fn pad_to_full_batch_window(prod: &mut rtrb::Producer<f32>, samples_already_pushed: usize) {
        let remaining = MAX_INFERENCE_WINDOW_SAMPLES.saturating_sub(samples_already_pushed);
        if remaining > 0 {
            push_samples(prod, 0.0, remaining);
        }
    }

    fn push_full_batch_utterance(prod: &mut rtrb::Producer<f32>, value: f32) {
        push_utterance(prod, value);
        pad_to_full_batch_window(prod, UTTERANCE_SAMPLES);
    }

    fn state_with(samples: &[f32]) -> PcmBufferState {
        PcmBufferState {
            samples: samples.iter().copied().collect(),
            samples_before_buffer: 0,
        }
    }

    fn tone(value: f32, count: usize) -> Vec<f32> {
        vec![value; count]
    }

    fn silence(count: usize) -> Vec<f32> {
        vec![0.0; count]
    }

    fn concat(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.iter().flatten().copied().collect()
    }

    #[test]
    fn first_cycle_not_ready_after_interval_with_partial_buffer() {
        let start = Instant::now() - BATCH_INTERVAL - Duration::from_millis(1);
        assert!(
            !first_cycle_ready(start, 200_000),
            "partial buffer must not trigger even after batch interval"
        );
    }

    #[test]
    fn first_cycle_ready_at_max_window_before_interval() {
        let start = Instant::now();
        assert!(
            first_cycle_ready(start, MAX_INFERENCE_WINDOW_SAMPLES),
            "480k samples must trigger first cycle without waiting"
        );
    }

    #[test]
    fn first_cycle_not_ready_with_zero_samples() {
        let start = Instant::now() - BATCH_INTERVAL - Duration::from_millis(1);
        assert!(
            !first_cycle_ready(start, 0),
            "empty buffer must not start a cycle even after interval"
        );
    }

    #[test]
    fn next_cycle_not_ready_after_interval_with_partial_buffer() {
        let completed = Instant::now() - BATCH_INTERVAL - Duration::from_millis(1);
        assert!(
            !next_cycle_ready(completed, 200_000, false),
            "partial buffer must not trigger even after batch interval"
        );
    }

    #[test]
    fn next_cycle_ready_immediately_when_full_window_backlog_remains() {
        let completed = Instant::now();
        assert!(
            next_cycle_ready(completed, MAX_INFERENCE_WINDOW_SAMPLES, true),
            "full-window backlog after previous cycle must skip batch interval"
        );
        assert!(
            !next_cycle_ready(completed, 100_000, true),
            "partial backlog must wait for another full window"
        );
    }

    #[test]
    fn next_cycle_ready_after_interval_with_full_window() {
        let completed = Instant::now() - BATCH_INTERVAL - Duration::from_millis(1);
        assert!(
            next_cycle_ready(completed, MAX_INFERENCE_WINDOW_SAMPLES, false),
            "full window after interval must start next cycle"
        );
    }

    #[test]
    fn next_cycle_not_ready_with_zero_samples_after_interval() {
        let completed = Instant::now() - BATCH_INTERVAL - Duration::from_millis(1);
        assert!(
            !next_cycle_ready(completed, 0, false),
            "empty buffer must not start next cycle"
        );
    }

    #[test]
    fn batch_worker_waits_for_full_window_before_first_inference() {
        let (sink, segments) = recording_sink();
        let inference_started = Arc::new(AtomicBool::new(false));
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "batch".to_string(),
                start_ms: 0,
                end_ms: 100,
            }],
            inference_started: Some(Arc::clone(&inference_started)),
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 10_000);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(&mut prod, 0.2, 50_000);
        thread::sleep(BATCH_INTERVAL + Duration::from_millis(20));
        assert!(
            !inference_started.load(Ordering::SeqCst),
            "partial buffer must not infer even after batch interval"
        );

        push_samples(&mut prod, 0.2, MAX_INFERENCE_WINDOW_SAMPLES - 50_000);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !inference_started.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(inference_started.load(Ordering::SeqCst));

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(segments.lock().expect("lock")[0].0, "batch");

        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn batch_worker_waits_interval_between_cycles() {
        let (sink, segments) = recording_sink();
        let inference_count = Arc::new(AtomicU64::new(0));
        let inference_count_capture = Arc::clone(&inference_count);

        struct CountingEngine {
            inner: MockEngine,
            count: Arc<AtomicU64>,
        }

        impl SegmentEngine for CountingEngine {
            fn transcribe_pcm(
                &mut self,
                pcm: &[f32],
            ) -> Result<Vec<WhisperSegment>, TranscribeError> {
                if !pcm.is_empty() {
                    self.count.fetch_add(1, Ordering::SeqCst);
                }
                self.inner.transcribe_pcm(pcm)
            }

            fn is_loaded(&self) -> bool {
                self.inner.is_loaded()
            }
        }

        impl ModelPathLoadable for CountingEngine {
            fn load_from_path_if_needed(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.inner.load_from_path_if_needed(path)
            }
        }

        let engine = CountingEngine {
            inner: MockEngine {
                segments: vec![WhisperSegment {
                    text: "cycle".to_string(),
                    start_ms: 0,
                    end_ms: 100,
                }],
                ..MockEngine::default()
            },
            count: inference_count_capture,
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(&mut prod, 0.3, MAX_INFERENCE_WINDOW_SAMPLES);

        let deadline = Instant::now() + Duration::from_secs(2);
        while inference_count.load(Ordering::SeqCst) < 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(inference_count.load(Ordering::SeqCst), 1);

        push_samples(&mut prod, 0.4, MAX_INFERENCE_WINDOW_SAMPLES);
        thread::sleep(BATCH_INTERVAL / 2);
        assert_eq!(
            inference_count.load(Ordering::SeqCst),
            1,
            "second cycle must wait for batch interval after first completes"
        );

        let deadline = Instant::now() + BATCH_INTERVAL * 3;
        while inference_count.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(inference_count.load(Ordering::SeqCst), 2);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").len() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(segments.lock().expect("lock").len(), 2);

        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn batch_worker_runs_continuous_cycles_on_backlog() {
        let (sink, segments) = recording_sink();
        let inference_count = Arc::new(AtomicU64::new(0));
        let inference_count_capture = Arc::clone(&inference_count);

        struct CountingEngine {
            inner: MockEngine,
            count: Arc<AtomicU64>,
        }

        impl SegmentEngine for CountingEngine {
            fn transcribe_pcm(
                &mut self,
                pcm: &[f32],
            ) -> Result<Vec<WhisperSegment>, TranscribeError> {
                if !pcm.is_empty() {
                    self.count.fetch_add(1, Ordering::SeqCst);
                }
                self.inner.transcribe_pcm(pcm)
            }

            fn is_loaded(&self) -> bool {
                self.inner.is_loaded()
            }
        }

        impl ModelPathLoadable for CountingEngine {
            fn load_from_path_if_needed(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.inner.load_from_path_if_needed(path)
            }
        }

        let engine = CountingEngine {
            inner: MockEngine {
                segments: vec![WhisperSegment {
                    text: "backlog".to_string(),
                    start_ms: 0,
                    end_ms: 100,
                }],
                ..MockEngine::default()
            },
            count: inference_count_capture,
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 100_000);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(
            &mut prod,
            0.5,
            MAX_INFERENCE_WINDOW_SAMPLES * 2,
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        while inference_count.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            inference_count.load(Ordering::SeqCst),
            2,
            "backlog must trigger immediate second cycle without waiting for batch interval"
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").len() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(segments.lock().expect("lock").len(), 2);

        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn stop_flush_transcribes_remaining_pcm_as_batch() {
        let (sink, segments) = recording_sink();
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "flushed".to_string(),
                start_ms: 0,
                end_ms: 100,
            }],
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(100_000);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(&mut prod, 0.2, 50_000);

        thread::sleep(BATCH_INTERVAL / 2);
        assert!(
            segments.lock().expect("lock").is_empty(),
            "partial buffer below interval must not infer before stop"
        );

        worker.stop_and_join(Duration::from_secs(2)).expect("stop");

        let recorded = segments.lock().expect("lock").clone();
        assert_eq!(recorded.len(), 1, "stop flush must transcribe remaining PCM");
        assert_eq!(recorded[0].0, "flushed");
    }

    #[test]
    fn inference_failure_continues_next_cycle() {
        let (sink, segments) = recording_sink();
        let attempt_count = Arc::new(AtomicU64::new(0));
        let attempt_count_capture = Arc::clone(&attempt_count);

        struct FailOnceEngine {
            attempts: Arc<AtomicU64>,
        }

        impl SegmentEngine for FailOnceEngine {
            fn transcribe_pcm(
                &mut self,
                pcm: &[f32],
            ) -> Result<Vec<WhisperSegment>, TranscribeError> {
                if pcm.is_empty() {
                    return Ok(Vec::new());
                }
                let n = self.attempts.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    return Err(TranscribeError::InferenceFailed {
                        detail: "injected failure".to_string(),
                    });
                }
                Ok(vec![WhisperSegment {
                    text: "recovered".to_string(),
                    start_ms: 0,
                    end_ms: 100,
                }])
            }

            fn is_loaded(&self) -> bool {
                true
            }
        }

        impl ModelPathLoadable for FailOnceEngine {
            fn load_from_path_if_needed(
                &mut self,
                _path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                Ok(())
            }
        }

        let mut worker = TranscribeWorker::with_engine(
            sink,
            FailOnceEngine {
                attempts: attempt_count_capture,
            },
        );
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 100_000);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(
            &mut prod,
            0.6,
            MAX_INFERENCE_WINDOW_SAMPLES * 2,
        );

        let deadline = Instant::now() + Duration::from_secs(3);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert!(
            !recorded.is_empty(),
            "worker must continue after inference failure"
        );
        assert_eq!(recorded[0].0, "recovered");
        assert!(
            attempt_count.load(Ordering::SeqCst) >= 2,
            "failed cycle must be followed by a retry on backlog"
        );

        worker.stop_and_join(Duration::from_secs(2)).expect("stop");
    }

    #[test]
    fn cuts_utterance_at_trailing_silence() {
        let mut state = state_with(&concat(&[tone(0.2, 32_000), silence(TEST_PAUSE_PADDING_SAMPLES)]));

        let (pcm, base) = take_window_from_state(&mut state, false).expect("window");

        assert_eq!(base, 0);
        assert_eq!(
            pcm.len(),
            32_000 + TRAILING_SILENCE_FRAMES * FRAME_SAMPLES,
            "window must end right after the closing pause"
        );
        assert_eq!(
            state.samples.len(),
            TEST_PAUSE_PADDING_SAMPLES - TRAILING_SILENCE_FRAMES * FRAME_SAMPLES,
            "extra silence stays for the next window"
        );
        assert_eq!(state.samples_before_buffer, pcm.len() as u64);
    }

    #[test]
    fn drops_leading_silence_and_offsets_base_timestamp() {
        let mut state = state_with(&concat(&[
            silence(16_000),
            tone(0.2, 32_000),
            silence(TEST_PAUSE_PADDING_SAMPLES),
        ]));

        let (pcm, base) = take_window_from_state(&mut state, false).expect("window");

        assert_eq!(base, 16_000, "base must skip the discarded leading silence");
        assert!(
            (pcm[0] - 0.2).abs() < f32::EPSILON,
            "window must start on speech"
        );
        assert_eq!(pcm.len(), 32_000 + TRAILING_SILENCE_FRAMES * FRAME_SAMPLES);
    }

    #[test]
    fn short_speech_waits_for_a_long_pause() {
        let mut state = state_with(&concat(&[tone(0.2, 8_000), silence(TEST_PAUSE_PADDING_SAMPLES)]));
        assert!(
            take_window_from_state(&mut state, false).is_none(),
            "a brief pause after short speech must not close the utterance"
        );

        state
            .samples
            .extend(silence(LONG_SILENCE_FRAMES * FRAME_SAMPLES - TEST_PAUSE_PADDING_SAMPLES));
        let (pcm, base) = take_window_from_state(&mut state, false).expect("window");
        assert_eq!(base, 0);
        assert_eq!(pcm.len(), 8_000 + LONG_SILENCE_FRAMES * FRAME_SAMPLES);
    }

    #[test]
    fn pure_silence_is_discarded_without_a_window() {
        let mut state = state_with(&silence(48_000));

        assert!(take_window_from_state(&mut state, false).is_none());
        assert!(state.samples.is_empty());
        assert_eq!(state.samples_before_buffer, 48_000);
    }

    #[test]
    fn forced_cut_lands_on_the_quietest_frame_of_the_last_two_seconds() {
        let mut pcm = tone(0.5, MAX_INFERENCE_WINDOW_SAMPLES);
        // A dip that is audible (above the silence threshold) ending at 29.0 s.
        let dip_end = 464_000;
        for sample in &mut pcm[dip_end - FRAME_SAMPLES..dip_end] {
            *sample = 0.02;
        }
        let mut state = state_with(&pcm);

        let (window, base) = take_window_from_state(&mut state, false).expect("window");

        assert_eq!(base, 0);
        assert_eq!(
            window.len(),
            dip_end,
            "cut must land after the quietest frame"
        );
        assert_eq!(state.samples.len(), MAX_INFERENCE_WINDOW_SAMPLES - dip_end);
    }

    #[test]
    fn forced_cut_never_exceeds_max_window() {
        let mut state = state_with(&tone(0.3, MAX_INFERENCE_WINDOW_SAMPLES + 32_000));

        let (window, _) = take_window_from_state(&mut state, false).expect("window");

        assert_eq!(window.len(), MAX_INFERENCE_WINDOW_SAMPLES);
        assert_eq!(state.samples.len(), 32_000);
    }

    #[test]
    fn take_batch_window_cuts_first_480k_samples() {
        let mut state = state_with(&tone(0.5, MAX_INFERENCE_WINDOW_SAMPLES + 100_000));

        let (pcm, base) = take_batch_window_from_state(&mut state).expect("window");

        assert_eq!(base, 0);
        assert_eq!(pcm.len(), MAX_INFERENCE_WINDOW_SAMPLES);
        assert_eq!(state.samples.len(), 100_000);
        assert_eq!(
            state.samples_before_buffer,
            MAX_INFERENCE_WINDOW_SAMPLES as u64
        );
    }

    #[test]
    fn take_batch_window_respects_base_offset() {
        let mut state = PcmBufferState {
            samples: tone(0.3, 100_000).into_iter().collect(),
            samples_before_buffer: 1_000_000,
        };

        let (pcm, base) = take_batch_window_from_state(&mut state).expect("window");

        assert_eq!(base, 1_000_000);
        assert_eq!(pcm.len(), 100_000);
        assert_eq!(state.samples_before_buffer, 1_100_000);
        assert!(state.samples.is_empty());
    }

    #[test]
    fn take_batch_window_returns_partial_when_below_max() {
        let mut state = state_with(&tone(0.2, 200_000));

        let (pcm, base) = take_batch_window_from_state(&mut state).expect("window");

        assert_eq!(base, 0);
        assert_eq!(pcm.len(), 200_000);
        assert!(state.samples.is_empty());
    }

    #[test]
    fn take_batch_window_returns_none_when_empty() {
        let mut state = state_with(&[]);
        assert!(take_batch_window_from_state(&mut state).is_none());
    }

    #[test]
    fn drain_consumer_retains_all_samples_past_old_buffer_cap() {
        let (mut prod, mut cons) = ring_pair(600_000);
        let mut state = PcmBufferState {
            samples: VecDeque::new(),
            samples_before_buffer: 0,
        };
        let push_count = 500_000usize;
        for i in 0..push_count {
            prod.push(i as f32 * 0.000_1).expect("push pcm");
        }

        let mut drained = 0usize;
        while drained < push_count {
            drained += drain_consumer(&mut cons, &mut state);
        }

        let retained = state.samples.len() + state.samples_before_buffer as usize;
        assert_eq!(
            drained,
            push_count,
            "drain must pop every sample from the ring buffer"
        );
        assert_eq!(
            retained,
            push_count,
            "no samples may be silently dropped when buffer exceeds old cap"
        );
    }

    #[test]
    fn flush_returns_unfinished_tail_only_when_long_enough() {
        let mut state = state_with(&tone(0.2, MIN_FLUSH_SAMPLES - FRAME_SAMPLES));
        assert!(take_window_from_state(&mut state, false).is_none());
        assert!(take_window_from_state(&mut state, true).is_none());

        let mut state = state_with(&tone(0.2, 12_800));
        assert!(take_window_from_state(&mut state, false).is_none());
        let (pcm, _) = take_window_from_state(&mut state, true).expect("flushed tail");
        assert_eq!(pcm.len(), 12_800);
        assert!(state.samples.is_empty());
    }

    type RecordedSegments = Arc<Mutex<Vec<(String, u64, String)>>>;

    fn recording_sink() -> (Arc<RecordingSink>, RecordedSegments) {
        let segments = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::new(RecordingSink {
            segments: Arc::clone(&segments),
        });
        (sink, segments)
    }

    #[test]
    fn spawn_requires_pcm_consumer() {
        let (sink, _) = recording_sink();
        let mut worker = TranscribeWorker::<MockEngine>::with_engine(sink, MockEngine::default());
        let err = worker.spawn().expect_err("missing consumer should fail");
        assert!(matches!(err, TranscribeError::Internal { .. }));
    }

    struct FailLoadEngine;

    impl SegmentEngine for FailLoadEngine {
        fn transcribe_pcm(&mut self, _pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
            Ok(Vec::new())
        }

        fn is_loaded(&self) -> bool {
            false
        }
    }

    impl ModelPathLoadable for FailLoadEngine {
        fn load_from_path_if_needed(
            &mut self,
            _path: &std::path::Path,
        ) -> Result<(), TranscribeError> {
            Err(TranscribeError::ModelCorrupt {
                detail: "forced load failure".to_string(),
            })
        }
    }

    #[test]
    fn load_failure_invokes_fatal_callback() {
        let (sink, _) = recording_sink();
        let mut worker = TranscribeWorker::<FailLoadEngine>::with_engine(sink, FailLoadEngine);
        let (producer, consumer) = ring_pair(32);
        drop(producer);
        worker.attach_pcm_consumer(consumer);
        let dummy = std::env::temp_dir().join("gijirec-fail-load-model.bin");
        std::fs::write(&dummy, b"not-a-model").expect("write dummy model file");
        worker
            .prepare_model_path(&dummy)
            .expect("prepare dummy path");
        let seen = Arc::new(Mutex::new(None));
        worker.set_fatal_error_callback({
            let seen = Arc::clone(&seen);
            Arc::new(move |err| {
                *seen.lock().expect("lock") = Some(err);
            })
        });
        worker.spawn().expect("spawn");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if seen.lock().expect("lock").is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fatal callback should run after load failure"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let err = seen.lock().expect("lock").take().expect("fatal error");
        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        let _ = worker.stop_and_join(Duration::from_secs(1));
        let _ = std::fs::remove_file(dummy);
    }

    #[test]
    fn engine_ready_callback_runs_after_successful_load() {
        let (sink, _) = recording_sink();
        let mut worker = TranscribeWorker::<MockEngine>::with_engine(sink, MockEngine::default());
        let (_prod, cons) = ring_pair(32);
        worker.attach_pcm_consumer(cons);
        let ready = Arc::new(AtomicBool::new(false));
        worker.set_engine_ready_callback({
            let ready = Arc::clone(&ready);
            Arc::new(move || ready.store(true, Ordering::SeqCst))
        });
        worker.spawn().expect("spawn");
        let deadline = Instant::now() + Duration::from_secs(2);
        while !ready.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(ready.load(Ordering::SeqCst));
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn spawn_starts_worker_thread() {
        let (sink, _) = recording_sink();
        let mut worker = TranscribeWorker::<MockEngine>::with_engine(sink, MockEngine::default());
        let (_prod, cons) = ring_pair(1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");
        assert!(worker.is_active());
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
        assert!(!worker.is_active());
    }

    #[test]
    fn stop_and_join_is_idempotent() {
        let (sink, _) = recording_sink();
        let mut worker = TranscribeWorker::<MockEngine>::with_engine(sink, MockEngine::default());
        let (_prod, cons) = ring_pair(1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");
        worker
            .stop_and_join(Duration::from_secs(1))
            .expect("first stop");
        worker
            .stop_and_join(Duration::from_secs(1))
            .expect("second stop");
        assert!(!worker.is_active());
    }

    #[test]
    fn lifecycle_delivers_segments_and_joins_cleanly() {
        let (sink, segments) = recording_sink();
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "hello".to_string(),
                start_ms: 100,
                end_ms: 500,
            }],
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_full_batch_utterance(&mut prod, 0.1);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "hello");
        assert_eq!(recorded[0].1, 100);
        assert_eq!(recorded[0].2, "auto");

        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
        assert!(!worker.is_active());
    }

    #[test]
    fn skips_whitespace_only_segments() {
        let (sink, segments) = recording_sink();
        let engine = MockEngine {
            segments: vec![
                WhisperSegment {
                    text: "   ".to_string(),
                    start_ms: 0,
                    end_ms: 100,
                },
                WhisperSegment {
                    text: "spoken".to_string(),
                    start_ms: 200,
                    end_ms: 400,
                },
            ],
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_full_batch_utterance(&mut prod, 0.2);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "spoken");
    }

    #[test]
    fn stop_timeout_detaches_without_internal_error() {
        let (sink, _) = recording_sink();
        let inference_started = Arc::new(AtomicBool::new(false));
        let engine = MockEngine {
            segments: vec![],
            inference_started: Some(Arc::clone(&inference_started)),
            block_until: Some(Arc::new(AtomicBool::new(false))),
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_full_batch_utterance(&mut prod, 0.3);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !inference_started.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(inference_started.load(Ordering::SeqCst));

        let result = worker.stop_and_join(Duration::from_millis(50));
        assert!(result.is_ok());
        if let Err(err) = result {
            assert_ne!(
                err.to_user_facing().code,
                TranscribeErrorCode::Internal,
                "timeout stop must not surface INTERNAL"
            );
        }
    }

    #[test]
    fn records_inference_latency_metric() {
        let (sink, _) = recording_sink();
        let called = Arc::new(AtomicBool::new(false));
        let called_capture = Arc::clone(&called);
        let latency_ms = Arc::new(AtomicU64::new(0));
        let latency_capture = Arc::clone(&latency_ms);
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "metric".to_string(),
                start_ms: 0,
                end_ms: 100,
            }],
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        worker.set_inference_latency_callback(Arc::new(move |ms| {
            called_capture.store(true, Ordering::SeqCst);
            latency_capture.store(ms, Ordering::SeqCst);
        }));
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_full_batch_utterance(&mut prod, 0.4);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !called.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        assert!(called.load(Ordering::SeqCst));
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn batch_cycle_observability_callbacks_record_started_and_completed() {
        let (sink, segments) = recording_sink();
        let started = Arc::new(Mutex::new(Vec::<BatchCycleStarted>::new()));
        let completed = Arc::new(Mutex::new(Vec::<BatchCycleCompleted>::new()));
        let started_capture = Arc::clone(&started);
        let completed_capture = Arc::clone(&completed);
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "observed".to_string(),
                start_ms: 0,
                end_ms: 100,
            }],
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        worker.set_batch_cycle_started_callback(Arc::new(move |event| {
            started_capture.lock().expect("lock").push(event);
            None
        }));
        worker.set_batch_cycle_completed_callback(Arc::new(move |event| {
            completed_capture.lock().expect("lock").push(event);
        }));
        let overflow = Arc::new(AtomicU64::new(3));
        worker.set_rtrb_overflow_counter(Arc::clone(&overflow));
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(&mut prod, 0.2, MAX_INFERENCE_WINDOW_SAMPLES);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        worker.stop_and_join(Duration::from_secs(1)).expect("stop");

        let started_events = started.lock().expect("lock");
        assert!(
            !started_events.is_empty(),
            "batch cycle started callback should fire"
        );
        assert_eq!(started_events[0].cycle_id, 1);
        assert_eq!(started_events[0].rtrb_overflow_count, 3);
        assert!(started_events[0].pcm_backlog_seconds > 0.0);

        let completed_events = completed.lock().expect("lock");
        assert!(
            !completed_events.is_empty(),
            "batch cycle completed callback should fire"
        );
        assert_eq!(completed_events[0].cycle_id, started_events[0].cycle_id);
        assert_eq!(completed_events[0].segments_count, 1);
    }

    #[test]
    fn continues_draining_pcm_while_inference_is_slow() {
        let (sink, segments) = recording_sink();
        let inference_started = Arc::new(AtomicBool::new(false));
        let unblock = Arc::new(AtomicBool::new(false));
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "during slow inference".to_string(),
                start_ms: 0,
                end_ms: 100,
            }],
            inference_started: Some(Arc::clone(&inference_started)),
            block_until: Some(Arc::clone(&unblock)),
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + UTTERANCE_SAMPLES);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        let pump = thread::spawn(move || {
            let mut pushed = 0usize;
            let target = MAX_INFERENCE_WINDOW_SAMPLES * 2;
            for _ in 0..target {
                prod.push(0.5)
                    .expect("pcm should keep draining during slow inference");
                pushed += 1;
            }
            pushed
        });

        let deadline = Instant::now() + Duration::from_secs(2);
        while !inference_started.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            inference_started.load(Ordering::SeqCst),
            "inference should start after first window"
        );

        thread::sleep(Duration::from_millis(100));
        unblock.store(true, Ordering::SeqCst);

        let pushed = pump.join().expect("pump thread");
        assert!(
            pushed > MAX_INFERENCE_WINDOW_SAMPLES,
            "expected to push more than one inference window while inference was blocked"
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(segments.lock().expect("lock")[0].0, "during slow inference");
        worker.stop_and_join(Duration::from_secs(2)).expect("stop");
    }

    #[test]
    fn skips_silent_windows_without_inference() {
        let (sink, segments) = recording_sink();
        let inference_started = Arc::new(AtomicBool::new(false));
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "should not appear".to_string(),
                start_ms: 0,
                end_ms: 100,
            }],
            inference_started: Some(Arc::clone(&inference_started)),
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        // A full max window of silence: must be discarded, never force-cut into inference.
        push_samples(&mut prod, 0.0, MAX_INFERENCE_WINDOW_SAMPLES);

        thread::sleep(Duration::from_millis(200));
        assert!(
            !inference_started.load(Ordering::SeqCst),
            "near-silence windows must not run whisper inference"
        );
        assert!(segments.lock().expect("lock").is_empty());
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn transcribes_first_window_when_inference_falls_behind() {
        let (sink, segments) = recording_sink();
        let inference_started = Arc::new(AtomicBool::new(false));
        let unblock = Arc::new(AtomicBool::new(false));
        let window_first_samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let window_first_samples_capture = Arc::clone(&window_first_samples);

        struct OrderedWindowEngine {
            inner: MockEngine,
            window_first_samples: Arc<Mutex<Vec<f32>>>,
        }

        impl SegmentEngine for OrderedWindowEngine {
            fn transcribe_pcm(
                &mut self,
                pcm: &[f32],
            ) -> Result<Vec<WhisperSegment>, TranscribeError> {
                if let Some(sample) = pcm.first() {
                    self.window_first_samples
                        .lock()
                        .expect("lock")
                        .push(*sample);
                }
                self.inner.transcribe_pcm(pcm)
            }

            fn is_loaded(&self) -> bool {
                self.inner.is_loaded()
            }
        }

        impl ModelPathLoadable for OrderedWindowEngine {
            fn load_from_path_if_needed(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.inner.load_from_path_if_needed(path)
            }
        }

        let engine = OrderedWindowEngine {
            inner: MockEngine {
                segments: vec![WhisperSegment {
                    text: "first".to_string(),
                    start_ms: 0,
                    end_ms: 100,
                }],
                inference_started: Some(Arc::clone(&inference_started)),
                block_until: Some(Arc::clone(&unblock)),
                ..MockEngine::default()
            },
            window_first_samples: window_first_samples_capture,
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 3 + UTTERANCE_SAMPLES);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_full_batch_utterance(&mut prod, 0.1);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !inference_started.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(inference_started.load(Ordering::SeqCst));

        // Two full windows arrive while inference is blocked: the first utterance must be kept.
        push_samples(&mut prod, 0.4, MAX_INFERENCE_WINDOW_SAMPLES);
        push_samples(&mut prod, 0.8, MAX_INFERENCE_WINDOW_SAMPLES);

        thread::sleep(Duration::from_millis(200));
        unblock.store(true, Ordering::SeqCst);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert!(!recorded.is_empty(), "expected inference on buffered audio");
        let ordered = window_first_samples.lock().expect("lock").clone();
        assert!(
            !ordered.is_empty(),
            "expected at least one inference window"
        );
        assert!(
            (ordered[0] - 0.1).abs() < f32::EPSILON,
            "behind worker must transcribe the first window in order, got first sample {}",
            ordered[0]
        );
        if ordered.len() >= 2 {
            assert!(
                (ordered[1] - 0.4).abs() < f32::EPSILON,
                "backlog catch-up must preserve FIFO order, got second sample {}",
                ordered[1]
            );
        }

        worker.stop_and_join(Duration::from_secs(2)).expect("stop");
    }

    #[test]
    fn double_spawn_is_rejected() {
        let (sink, _) = recording_sink();
        let mut worker = TranscribeWorker::<MockEngine>::with_engine(sink, MockEngine::default());
        let (_prod, cons) = ring_pair(1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("first spawn");
        let err = worker.spawn().expect_err("second spawn");
        assert!(matches!(err, TranscribeError::Internal { .. }));
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    }

    #[test]
    fn run_inference_window_reports_level_before_skip_or_inference() {
        let (sink, _) = recording_sink();
        let sink_trait: Arc<dyn TranscriptSegmentSink> = sink;
        let mut engine = MockEngine::default();
        let levels: Arc<Mutex<Vec<InferenceWindowLevel>>> = Arc::new(Mutex::new(Vec::new()));
        let levels_cb: InferenceWindowLevelCallback = Arc::new({
            let levels = Arc::clone(&levels);
            move |level| levels.lock().expect("lock").push(level)
        });
        let ctx = InferenceContext {
            engine: &mut engine,
            sink: &sink_trait,
            on_latency: None,
            on_attempted: None,
            on_window_level: Some(&levels_cb),
        };

        let silent = vec![0.0_f32; 1_600];
        run_inference_window(&silent, 0, ctx);
        let recorded = levels.lock().expect("lock");
        assert_eq!(recorded.len(), 1);
        assert!(recorded[0].inference_skipped);
        assert!(recorded[0].window_rms < SILENCE_RMS_THRESHOLD);
    }

    #[test]
    fn run_inference_window_timestamp_uses_batch_window_front_plus_segment_offset() {
        let (sink, segments) = recording_sink();
        let sink_trait: Arc<dyn TranscriptSegmentSink> = sink;
        let mut engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "timed".to_string(),
                start_ms: 500,
                end_ms: 1_000,
            }],
            ..MockEngine::default()
        };
        let ctx = InferenceContext {
            engine: &mut engine,
            sink: &sink_trait,
            on_latency: None,
            on_attempted: None,
            on_window_level: None,
        };
        let pcm = tone(0.5, 1_600);
        run_inference_window(&pcm, 160_000, ctx);

        let recorded = segments.lock().expect("lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(
            recorded[0].1,
            10_500,
            "start_ms must be samples_before_buffer (10_000 ms) + segment offset (500 ms)"
        );
    }

    #[test]
    fn batch_worker_emits_start_timestamp_from_batch_window_base() {
        let (sink, segments) = recording_sink();
        let engine = MockEngine {
            segments: vec![WhisperSegment {
                text: "offset batch".to_string(),
                start_ms: 500,
                end_ms: 2_000,
            }],
            ..MockEngine::default()
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        // Advance samples_before_buffer with a silent full window, then infer speech at 30 s offset.
        push_samples(&mut prod, 0.0, MAX_INFERENCE_WINDOW_SAMPLES);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            segments.lock().expect("lock").is_empty(),
            "silent full window must not emit transcript blocks"
        );

        push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES);

        let deadline = Instant::now() + Duration::from_secs(3);
        while segments.lock().expect("lock").is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "offset batch");
        assert_eq!(
            recorded[0].1,
            30_500,
            "timestamp must use batch window front (30_000 ms) + segment offset (500 ms)"
        );

        worker.stop_and_join(Duration::from_secs(2)).expect("stop");
    }

    #[test]
    fn start_timestamp_ms_is_monotonic_across_batch_cycles() {
        let (sink, segments) = recording_sink();
        let cycle = Arc::new(AtomicUsize::new(0));
        let cycle_capture = Arc::clone(&cycle);

        struct MultiCycleEngine {
            cycle: Arc<AtomicUsize>,
        }

        impl SegmentEngine for MultiCycleEngine {
            fn transcribe_pcm(
                &mut self,
                _pcm: &[f32],
            ) -> Result<Vec<WhisperSegment>, TranscribeError> {
                let index = self.cycle.fetch_add(1, Ordering::SeqCst);
                let base_ms = index as i64 * 30_000;
                Ok(vec![WhisperSegment {
                    text: format!("cycle-{index}"),
                    start_ms: base_ms + 100,
                    end_ms: base_ms + 500,
                }])
            }

            fn is_loaded(&self) -> bool {
                true
            }
        }

        impl ModelPathLoadable for MultiCycleEngine {
            fn load_from_path_if_needed(
                &mut self,
                _path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                Ok(())
            }
        }

        let mut worker = TranscribeWorker::with_engine(
            sink,
            MultiCycleEngine {
                cycle: cycle_capture,
            },
        );
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES * 2);

        let deadline = Instant::now() + Duration::from_secs(3);
        while segments.lock().expect("lock").len() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert_eq!(recorded.len(), 2);
        assert!(
            recorded[1].1 > recorded[0].1,
            "start_timestamp_ms must increase across consecutive batch cycles"
        );

        worker.stop_and_join(Duration::from_secs(2)).expect("stop");
    }

    #[test]
    fn deferred_variant_reload_applies_on_next_batch_cycle() {
        let (sink, _) = recording_sink();
        let loaded_paths = Arc::new(Mutex::new(Vec::<std::path::PathBuf>::new()));
        let loaded_paths_capture = Arc::clone(&loaded_paths);
        let cycle_count = Arc::new(AtomicU64::new(0));
        let cycle_count_capture = Arc::clone(&cycle_count);
        let pending_reload = Arc::new(Mutex::new(None::<std::path::PathBuf>));
        let pending_reload_capture = Arc::clone(&pending_reload);

        struct PathTrackingEngine {
            paths: Arc<Mutex<Vec<std::path::PathBuf>>>,
        }

        impl SegmentEngine for PathTrackingEngine {
            fn transcribe_pcm(&mut self, _pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
                Ok(Vec::new())
            }

            fn is_loaded(&self) -> bool {
                true
            }
        }

        impl ModelPathLoadable for PathTrackingEngine {
            fn load_from_path_if_needed(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.paths.lock().expect("lock").push(path.to_path_buf());
                Ok(())
            }

            fn reload_from_path(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.paths.lock().expect("lock").push(path.to_path_buf());
                Ok(())
            }
        }

        let next_path = std::path::PathBuf::from("/tmp/models/q8.bin");

        let mut worker = TranscribeWorker::with_engine(
            sink,
            PathTrackingEngine {
                paths: loaded_paths_capture,
            },
        );
        worker.set_batch_cycle_started_callback(Arc::new({
            let cycle_count = cycle_count_capture;
            let pending_reload = pending_reload_capture;
            move |_event| {
                let cycle = cycle_count.fetch_add(1, Ordering::SeqCst) + 1;
                if cycle == 2 {
                    return pending_reload.lock().expect("lock").clone();
                }
                None
            }
        }));
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES);

        let deadline = Instant::now() + Duration::from_secs(2);
        while cycle_count.load(Ordering::SeqCst) < 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(cycle_count.load(Ordering::SeqCst), 1);

        *pending_reload.lock().expect("lock") = Some(next_path.clone());

        push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES);

        let deadline = Instant::now() + Duration::from_secs(3);
        while cycle_count.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }

        let paths = loaded_paths.lock().expect("lock").clone();
        assert_eq!(paths.len(), 1, "only cycle-boundary reload should record a path");
        assert_eq!(paths[0], next_path, "second cycle must reload deferred model path");

        worker.stop_and_join(Duration::from_secs(2)).expect("stop");
    }
}
