use std::collections::VecDeque;

use gijirec_domain::transcribe::TranscribeErrorCode;
use rtrb::RingBuffer;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::transcribe::whisper_adapter::WhisperSegment;
use gijirec_domain::transcribe::{TranscribeError, TranscriptSegmentSink};

use super::batch_cycle::{InferenceContext, SILENCE_RMS_THRESHOLD, run_inference_window};
use super::batch_window::{
    BATCH_INTERVAL, MAX_INFERENCE_WINDOW_SAMPLES, first_cycle_ready, next_cycle_ready,
    take_batch_window_from_state,
};
use super::pcm_buffer::{PcmBufferState, drain_consumer};
use super::types::InferenceWindowLevelCallback;
use super::{
    BatchCycleCompleted, BatchCycleStarted, InferenceWindowLevel, ModelPathLoadable, SegmentEngine,
    TranscribeWorker,
};

use crate::noop_model_path_loadable;

macro_rules! delegate_model_path_loadable {
    ($ty:ty) => {
        impl ModelPathLoadable for $ty {
            fn load_from_path_if_needed(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.inner.load_from_path_if_needed(path)
            }
        }
    };
}

type RecordedSegments = Arc<Mutex<Vec<(String, u64, String)>>>;

struct RecordingSink {
    segments: RecordedSegments,
}

impl TranscriptSegmentSink for RecordingSink {
    fn on_segment(&self, text: &str, start_ms: u64, language: &str) -> Result<(), TranscribeError> {
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
    fn load_from_path_if_needed(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        self.loaded_path = Some(path.to_path_buf());
        Ok(())
    }

    fn reload_from_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        self.loaded_path = Some(path.to_path_buf());
        Ok(())
    }
}

fn ring_pair(capacity: usize) -> (rtrb::Producer<f32>, rtrb::Consumer<f32>) {
    RingBuffer::<f32>::new(capacity)
}

fn push_samples(prod: &mut rtrb::Producer<f32>, value: f32, count: usize) {
    let samples = vec![value; count];
    prod.push_entire_slice(&samples).expect("push pcm");
}

const DEFAULT_TEST_WAIT: Duration = Duration::from_secs(2);

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !condition() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_counter(counter: &AtomicU64, at_least: u64) {
    wait_until(DEFAULT_TEST_WAIT, || {
        counter.load(Ordering::SeqCst) >= at_least
    });
}

fn wait_for_counter_timeout(counter: &AtomicU64, at_least: u64, timeout: Duration) {
    wait_until(timeout, || counter.load(Ordering::SeqCst) >= at_least);
}

fn wait_for_bool(flag: &AtomicBool) {
    wait_until(DEFAULT_TEST_WAIT, || flag.load(Ordering::SeqCst));
}

fn wait_for_segments_at_least(segments: &RecordedSegments, len: usize) {
    wait_for_segments_at_least_timeout(segments, len, DEFAULT_TEST_WAIT);
}

fn assert_segments_len(segments: &RecordedSegments, len: usize) {
    wait_for_segments_at_least(segments, len);
    assert_eq!(segments.lock().expect("lock").len(), len);
}

fn wait_for_segments_at_least_timeout(segments: &RecordedSegments, len: usize, timeout: Duration) {
    wait_until(timeout, || segments.lock().expect("lock").len() >= len);
}

fn wait_for_mutex_some<T>(slot: &Arc<Mutex<Option<T>>>) {
    wait_until(DEFAULT_TEST_WAIT, || slot.lock().expect("lock").is_some());
}

fn batch_text_engine(text: &str, inference_started: Arc<AtomicBool>) -> MockEngine {
    MockEngine {
        segments: vec![whisper_segment(text, 0, 100)],
        inference_started: Some(inference_started),
        ..MockEngine::default()
    }
}

struct CountingEngine {
    inner: MockEngine,
    count: Arc<AtomicU64>,
}

impl SegmentEngine for CountingEngine {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        if !pcm.is_empty() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
        self.inner.transcribe_pcm(pcm)
    }

    fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }
}

delegate_model_path_loadable!(CountingEngine);

fn counting_text_engine(text: &str, count: Arc<AtomicU64>) -> CountingEngine {
    CountingEngine {
        inner: mock_engine_with_segments(vec![whisper_segment(text, 0, 100)]),
        count,
    }
}

/// Pushes one full 30 s batch window of PCM.
fn push_full_batch_window(prod: &mut rtrb::Producer<f32>, value: f32) {
    push_samples(prod, value, MAX_INFERENCE_WINDOW_SAMPLES);
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
    let inference_started = Arc::new(AtomicBool::new(false));
    let engine = batch_text_engine("batch", Arc::clone(&inference_started));
    let (mut worker, mut prod, segments) =
        spawn_batch_worker(engine, MAX_INFERENCE_WINDOW_SAMPLES + 10_000);

    push_samples(&mut prod, 0.2, 50_000);
    thread::sleep(BATCH_INTERVAL + Duration::from_millis(20));
    assert!(
        !inference_started.load(Ordering::SeqCst),
        "partial buffer must not infer even after batch interval"
    );

    push_samples(&mut prod, 0.2, MAX_INFERENCE_WINDOW_SAMPLES - 50_000);

    wait_until(Duration::from_secs(2), || {
        inference_started.load(Ordering::SeqCst)
    });
    assert!(inference_started.load(Ordering::SeqCst));

    wait_until(Duration::from_secs(2), || {
        !segments.lock().expect("lock").is_empty()
    });
    assert_eq!(segments.lock().expect("lock")[0].0, "batch");

    worker.stop_and_join(Duration::from_secs(1)).expect("stop");
}

#[test]
fn batch_worker_waits_interval_between_cycles() {
    let inference_count = Arc::new(AtomicU64::new(0));
    let engine = counting_text_engine("cycle", Arc::clone(&inference_count));
    let (mut worker, mut prod, segments) =
        spawn_batch_worker(engine, MAX_INFERENCE_WINDOW_SAMPLES * 2);

    push_samples(&mut prod, 0.3, MAX_INFERENCE_WINDOW_SAMPLES);

    wait_for_counter(&inference_count, 1);
    assert_eq!(inference_count.load(Ordering::SeqCst), 1);

    push_samples(&mut prod, 0.4, MAX_INFERENCE_WINDOW_SAMPLES);
    thread::sleep(BATCH_INTERVAL / 2);
    assert_eq!(
        inference_count.load(Ordering::SeqCst),
        1,
        "second cycle must wait for batch interval after first completes"
    );

    wait_for_counter_timeout(&inference_count, 2, BATCH_INTERVAL * 3);
    assert_eq!(inference_count.load(Ordering::SeqCst), 2);

    assert_segments_len(&segments, 2);
    stop_worker_inactive(&mut worker);
}

#[test]
fn batch_worker_runs_continuous_cycles_on_backlog() {
    let inference_count = Arc::new(AtomicU64::new(0));
    let engine = counting_text_engine("backlog", Arc::clone(&inference_count));
    let (mut worker, mut prod, segments) =
        spawn_batch_worker(engine, MAX_INFERENCE_WINDOW_SAMPLES * 2 + 100_000);

    push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES * 2);

    wait_for_counter(&inference_count, 2);
    assert_eq!(
        inference_count.load(Ordering::SeqCst),
        2,
        "backlog must trigger immediate second cycle without waiting for batch interval"
    );

    assert_segments_len(&segments, 2);
    stop_worker_inactive(&mut worker);
}

#[test]
fn stop_flush_transcribes_remaining_pcm_as_batch() {
    let (mut worker, mut prod, segments) = spawn_mock_worker(
        mock_engine_with_segments(vec![whisper_segment("flushed", 0, 100)]),
        100_000,
    );

    push_samples(&mut prod, 0.2, 50_000);

    thread::sleep(BATCH_INTERVAL / 2);
    assert!(
        segments.lock().expect("lock").is_empty(),
        "partial buffer below interval must not infer before stop"
    );

    worker.stop_and_join(Duration::from_secs(2)).expect("stop");

    let recorded = segments.lock().expect("lock").clone();
    assert_eq!(
        recorded.len(),
        1,
        "stop flush must transcribe remaining PCM"
    );
    assert_eq!(recorded[0].0, "flushed");
}

struct FailOnceEngine {
    attempts: Arc<AtomicU64>,
}

impl SegmentEngine for FailOnceEngine {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }
        let n = self.attempts.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            return Err(TranscribeError::InferenceFailed {
                detail: "injected failure".to_string(),
            });
        }
        Ok(vec![whisper_segment("recovered", 0, 100)])
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

noop_model_path_loadable!(FailOnceEngine);

#[test]
fn inference_failure_continues_next_cycle() {
    let (sink, segments) = recording_sink();
    let attempt_count = Arc::new(AtomicU64::new(0));
    let attempt_count_capture = Arc::clone(&attempt_count);

    let mut worker = TranscribeWorker::with_engine(
        sink,
        FailOnceEngine {
            attempts: attempt_count_capture,
        },
    );
    let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 100_000);
    worker.attach_pcm_consumer(cons);
    worker.spawn().expect("spawn");

    push_samples(&mut prod, 0.6, MAX_INFERENCE_WINDOW_SAMPLES * 2);

    wait_for_segments_at_least_timeout(&segments, 1, Duration::from_secs(3));

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
        drained, push_count,
        "drain must pop every sample from the ring buffer"
    );
    assert_eq!(
        retained, push_count,
        "no samples may be silently dropped when buffer exceeds old cap"
    );
}

fn recording_sink() -> (Arc<RecordingSink>, RecordedSegments) {
    let segments = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(RecordingSink {
        segments: Arc::clone(&segments),
    });
    (sink, segments)
}

fn spawn_batch_worker<E: SegmentEngine + ModelPathLoadable + 'static>(
    engine: E,
    ring_capacity: usize,
) -> (TranscribeWorker<E>, rtrb::Producer<f32>, RecordedSegments) {
    let (sink, segments) = recording_sink();
    let mut worker = TranscribeWorker::with_engine(sink, engine);
    let (prod, cons) = ring_pair(ring_capacity);
    worker.attach_pcm_consumer(cons);
    worker.spawn().expect("spawn");
    (worker, prod, segments)
}

fn whisper_segment(text: &str, start_ms: i64, end_ms: i64) -> WhisperSegment {
    WhisperSegment {
        text: text.to_string(),
        start_ms,
        end_ms,
    }
}

fn mock_engine_with_segments(segments: Vec<WhisperSegment>) -> MockEngine {
    MockEngine {
        segments,
        ..MockEngine::default()
    }
}

fn spawn_mock_worker(
    engine: MockEngine,
    ring_capacity: usize,
) -> (
    TranscribeWorker<MockEngine>,
    rtrb::Producer<f32>,
    RecordedSegments,
) {
    spawn_batch_worker(engine, ring_capacity)
}

fn stop_worker_inactive<E: SegmentEngine + 'static>(worker: &mut TranscribeWorker<E>) {
    worker.stop_and_join(Duration::from_secs(1)).expect("stop");
    assert!(!worker.is_active());
}

fn assert_recorded_segments(segments: &RecordedSegments, expected: &[(&str, u64, &str)]) {
    let recorded = segments.lock().expect("lock").clone();
    assert_eq!(recorded.len(), expected.len());
    for (actual, (text, start_ms, language)) in recorded.iter().zip(expected) {
        assert_eq!(actual.0, *text);
        assert_eq!(actual.1, *start_ms);
        assert_eq!(actual.2, *language);
    }
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
    fn load_from_path_if_needed(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
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
    wait_for_mutex_some(&seen);
    assert!(
        seen.lock().expect("lock").is_some(),
        "fatal callback should run after load failure"
    );
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
    wait_for_bool(&ready);
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
    let (mut worker, mut prod, segments) = spawn_mock_worker(
        mock_engine_with_segments(vec![whisper_segment("hello", 100, 500)]),
        MAX_INFERENCE_WINDOW_SAMPLES + 1_024,
    );

    push_full_batch_window(&mut prod, 0.1);

    wait_for_segments_at_least(&segments, 1);
    assert_recorded_segments(&segments, &[("hello", 100, "auto")]);
    stop_worker_inactive(&mut worker);
}

#[test]
fn skips_whitespace_only_segments() {
    let (mut worker, mut prod, segments) = spawn_mock_worker(
        mock_engine_with_segments(vec![
            whisper_segment("   ", 0, 100),
            whisper_segment("spoken", 200, 400),
        ]),
        MAX_INFERENCE_WINDOW_SAMPLES + 1_024,
    );

    push_full_batch_window(&mut prod, 0.2);

    wait_for_segments_at_least(&segments, 1);
    assert_recorded_segments(&segments, &[("spoken", 200, "auto")]);
    stop_worker_inactive(&mut worker);
}

#[test]
fn stop_timeout_detaches_without_internal_error() {
    let inference_started = Arc::new(AtomicBool::new(false));
    let engine = MockEngine {
        segments: vec![],
        inference_started: Some(Arc::clone(&inference_started)),
        block_until: Some(Arc::new(AtomicBool::new(false))),
        ..MockEngine::default()
    };
    let (mut worker, mut prod, _) = spawn_mock_worker(engine, MAX_INFERENCE_WINDOW_SAMPLES + 1_024);

    push_full_batch_window(&mut prod, 0.3);

    wait_for_bool(&inference_started);
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
    let mut worker = TranscribeWorker::with_engine(
        sink,
        mock_engine_with_segments(vec![whisper_segment("metric", 0, 100)]),
    );
    worker.set_inference_latency_callback(Arc::new(move |ms| {
        called_capture.store(true, Ordering::SeqCst);
        latency_capture.store(ms, Ordering::SeqCst);
    }));
    let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES + 1_024);
    worker.attach_pcm_consumer(cons);
    worker.spawn().expect("spawn");

    push_full_batch_window(&mut prod, 0.4);

    wait_for_bool(&called);

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
    let engine = mock_engine_with_segments(vec![whisper_segment("observed", 0, 100)]);
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

    wait_for_segments_at_least(&segments, 1);

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
        segments: vec![whisper_segment("during slow inference", 0, 100)],
        inference_started: Some(Arc::clone(&inference_started)),
        block_until: Some(Arc::clone(&unblock)),
        ..MockEngine::default()
    };
    let mut worker = TranscribeWorker::with_engine(sink, engine);
    let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 1_024);
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

    wait_for_bool(&inference_started);
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

    wait_for_segments_at_least(&segments, 1);

    assert_eq!(segments.lock().expect("lock")[0].0, "during slow inference");
    worker.stop_and_join(Duration::from_secs(2)).expect("stop");
}

#[test]
fn skips_silent_windows_without_inference() {
    let inference_started = Arc::new(AtomicBool::new(false));
    let engine = MockEngine {
        segments: vec![whisper_segment("should not appear", 0, 100)],
        inference_started: Some(Arc::clone(&inference_started)),
        ..MockEngine::default()
    };
    let (mut worker, mut prod, segments) =
        spawn_mock_worker(engine, MAX_INFERENCE_WINDOW_SAMPLES + 1_024);

    // A full max window of silence: RMS skip must avoid whisper inference.
    push_samples(&mut prod, 0.0, MAX_INFERENCE_WINDOW_SAMPLES);

    thread::sleep(Duration::from_millis(200));
    assert!(
        !inference_started.load(Ordering::SeqCst),
        "near-silence windows must not run whisper inference"
    );
    assert!(segments.lock().expect("lock").is_empty());
    worker.stop_and_join(Duration::from_secs(1)).expect("stop");
}

struct OrderedWindowEngine {
    inner: MockEngine,
    window_first_samples: Arc<Mutex<Vec<f32>>>,
}

impl SegmentEngine for OrderedWindowEngine {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
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

delegate_model_path_loadable!(OrderedWindowEngine);

#[test]
fn transcribes_first_window_when_inference_falls_behind() {
    let (sink, segments) = recording_sink();
    let inference_started = Arc::new(AtomicBool::new(false));
    let unblock = Arc::new(AtomicBool::new(false));
    let window_first_samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let window_first_samples_capture = Arc::clone(&window_first_samples);

    let engine = OrderedWindowEngine {
        inner: MockEngine {
            segments: vec![whisper_segment("first", 0, 100)],
            inference_started: Some(Arc::clone(&inference_started)),
            block_until: Some(Arc::clone(&unblock)),
            ..MockEngine::default()
        },
        window_first_samples: window_first_samples_capture,
    };
    let mut worker = TranscribeWorker::with_engine(sink, engine);
    let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 3 + 1_024);
    worker.attach_pcm_consumer(cons);
    worker.spawn().expect("spawn");

    push_full_batch_window(&mut prod, 0.1);

    wait_for_bool(&inference_started);
    assert!(inference_started.load(Ordering::SeqCst));

    // Two full windows arrive while inference is blocked: the first utterance must be kept.
    push_samples(&mut prod, 0.4, MAX_INFERENCE_WINDOW_SAMPLES);
    push_samples(&mut prod, 0.8, MAX_INFERENCE_WINDOW_SAMPLES);

    thread::sleep(Duration::from_millis(200));
    unblock.store(true, Ordering::SeqCst);

    wait_for_segments_at_least(&segments, 1);

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
    let mut engine = mock_engine_with_segments(vec![whisper_segment("timed", 500, 1_000)]);
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
        recorded[0].1, 10_500,
        "start_ms must be samples_before_buffer (10_000 ms) + segment offset (500 ms)"
    );
}

#[test]
fn batch_worker_emits_start_timestamp_from_batch_window_base() {
    let (mut worker, mut prod, segments) = spawn_mock_worker(
        mock_engine_with_segments(vec![whisper_segment("offset batch", 500, 2_000)]),
        MAX_INFERENCE_WINDOW_SAMPLES * 2,
    );

    // Advance samples_before_buffer with a silent full window, then infer speech at 30 s offset.
    push_samples(&mut prod, 0.0, MAX_INFERENCE_WINDOW_SAMPLES);

    wait_for_segments_at_least(&segments, 1);
    assert!(
        segments.lock().expect("lock").is_empty(),
        "silent full window must not emit transcript blocks"
    );

    push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES);

    wait_for_segments_at_least_timeout(&segments, 1, Duration::from_secs(3));

    let recorded = segments.lock().expect("lock").clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].0, "offset batch");
    assert_eq!(
        recorded[0].1, 30_500,
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
        fn transcribe_pcm(&mut self, _pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
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

    noop_model_path_loadable!(MultiCycleEngine);

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

    wait_for_segments_at_least_timeout(&segments, 2, Duration::from_secs(3));

    let recorded = segments.lock().expect("lock").clone();
    assert_eq!(recorded.len(), 2);
    assert!(
        recorded[1].1 > recorded[0].1,
        "start_timestamp_ms must increase across consecutive batch cycles"
    );

    worker.stop_and_join(Duration::from_secs(2)).expect("stop");
}

fn deferred_reload_path_on_cycle_two(
    cycle_count: &AtomicU64,
    pending_reload: &Mutex<Option<std::path::PathBuf>>,
    _event: BatchCycleStarted,
) -> Option<std::path::PathBuf> {
    let cycle = cycle_count.fetch_add(1, Ordering::SeqCst) + 1;
    if cycle == 2 {
        return pending_reload.lock().expect("lock").clone();
    }
    None
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

        fn reload_from_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
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
        move |event| deferred_reload_path_on_cycle_two(&cycle_count, &pending_reload, event)
    }));
    let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 2 + 1_024);
    worker.attach_pcm_consumer(cons);
    worker.spawn().expect("spawn");

    push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES);

    wait_for_counter(&cycle_count, 1);
    assert_eq!(cycle_count.load(Ordering::SeqCst), 1);

    *pending_reload.lock().expect("lock") = Some(next_path.clone());

    push_samples(&mut prod, 0.5, MAX_INFERENCE_WINDOW_SAMPLES);

    wait_for_counter_timeout(&cycle_count, 2, Duration::from_secs(3));

    let paths = loaded_paths.lock().expect("lock").clone();
    assert_eq!(
        paths.len(),
        1,
        "only cycle-boundary reload should record a path"
    );
    assert_eq!(
        paths[0], next_path,
        "second cycle must reload deferred model path"
    );

    worker.stop_and_join(Duration::from_secs(2)).expect("stop");
}
