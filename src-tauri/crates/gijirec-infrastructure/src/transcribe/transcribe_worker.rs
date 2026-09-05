//! Dedicated-thread PCM consumer and VAD-driven whisper inference worker.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use gijirec_domain::transcribe::{TranscribeError, TranscriptSegmentSink};

use super::whisper_adapter::{WhisperCppAdapter, WhisperSegment};

/// Maximum accumulated PCM before dropping oldest samples (30 s @ 16 kHz).
pub const MAX_PCM_BUFFER_SAMPLES: usize = 480_000;

/// Trigger inference after this many new samples (~5 s @ 16 kHz).
const INFERENCE_WINDOW_SAMPLES: usize = 80_000;

const SAMPLE_RATE_HZ: u64 = 16_000;

/// Inference engine abstraction (production: [`WhisperCppAdapter`], tests: mocks).
pub trait SegmentEngine: Send {
    fn transcribe_pcm(&self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError>;
    fn is_loaded(&self) -> bool;
}

impl SegmentEngine for WhisperCppAdapter {
    fn transcribe_pcm(&self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        WhisperCppAdapter::transcribe_pcm(self, pcm)
    }

    fn is_loaded(&self) -> bool {
        WhisperCppAdapter::is_loaded(self)
    }
}

/// VAD-driven inference worker reading PCM from an rtrb consumer.
pub struct TranscribeWorker<E: SegmentEngine + 'static = WhisperCppAdapter> {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pcm_consumer: Option<rtrb::Consumer<f32>>,
    engine: Option<E>,
    sink: Arc<dyn TranscriptSegmentSink>,
    on_inference_latency_ms: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}

impl<E: SegmentEngine + 'static> TranscribeWorker<E> {
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
            sink,
            on_inference_latency_ms: None,
        }
    }

    pub fn install_engine(&mut self, engine: E) {
        self.engine = Some(engine);
    }

    pub fn attach_pcm_consumer(&mut self, consumer: rtrb::Consumer<f32>) {
        self.pcm_consumer = Some(consumer);
    }

    pub fn set_inference_latency_callback(&mut self, callback: Arc<dyn Fn(u64) + Send + Sync>) {
        self.on_inference_latency_ms = Some(callback);
    }

    pub fn is_active(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn spawn(&mut self) -> Result<(), TranscribeError> {
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
        let engine = self
            .engine
            .take()
            .ok_or_else(|| TranscribeError::Internal {
                detail: "inference engine not available".to_string(),
            })?;

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let sink = Arc::clone(&self.sink);
        let on_latency = self.on_inference_latency_ms.clone();

        let params = WorkerParams {
            consumer,
            engine,
            sink,
            running,
            on_latency,
        };
        let handle = thread::Builder::new()
            .name("transcribe-worker".into())
            .spawn(move || {
                set_worker_thread_priority();
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
    sink: Arc<dyn TranscriptSegmentSink>,
    running: Arc<AtomicBool>,
    on_latency: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}

struct InferenceContext<'a, E> {
    engine: &'a E,
    sink: &'a Arc<dyn TranscriptSegmentSink>,
    on_latency: Option<&'a Arc<dyn Fn(u64) + Send + Sync>>,
}

fn worker_loop<E: SegmentEngine>(params: WorkerParams<E>) {
    let WorkerParams {
        mut consumer,
        engine,
        sink,
        running,
        on_latency,
    } = params;
    let mut buffer = Vec::new();
    let mut samples_before_buffer: u64 = 0;

    let ctx = InferenceContext {
        engine: &engine,
        sink: &sink,
        on_latency: on_latency.as_ref(),
    };

    while running.load(Ordering::SeqCst) {
        drain_consumer(&mut consumer, &mut buffer, &mut samples_before_buffer);

        if buffer.len() >= INFERENCE_WINDOW_SAMPLES && engine.is_loaded() {
            run_inference(&mut buffer, &mut samples_before_buffer, &ctx);
        } else if buffer.is_empty() {
            thread::sleep(Duration::from_millis(5));
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }

    drain_consumer(&mut consumer, &mut buffer, &mut samples_before_buffer);
    if !buffer.is_empty() && engine.is_loaded() {
        run_inference(&mut buffer, &mut samples_before_buffer, &ctx);
    }

    drop(engine);
}

fn drain_consumer(
    consumer: &mut rtrb::Consumer<f32>,
    buffer: &mut Vec<f32>,
    samples_before_buffer: &mut u64,
) {
    while let Ok(sample) = consumer.pop() {
        buffer.push(sample);
        if buffer.len() > MAX_PCM_BUFFER_SAMPLES {
            let drop_count = buffer.len() - MAX_PCM_BUFFER_SAMPLES;
            buffer.drain(..drop_count);
            *samples_before_buffer += drop_count as u64;
        }
    }
}

fn run_inference<E: SegmentEngine>(
    buffer: &mut Vec<f32>,
    samples_before_buffer: &mut u64,
    ctx: &InferenceContext<'_, E>,
) {
    if buffer.is_empty() {
        return;
    }

    let inference_start = Instant::now();
    let base_ms = samples_to_ms(*samples_before_buffer);

    if let Ok(segments) = ctx.engine.transcribe_pcm(buffer) {
        for segment in segments {
            let trimmed = segment.text.trim();
            if trimmed.is_empty() {
                continue;
            }
            let start_ms = base_ms.saturating_add(segment.start_ms.max(0) as u64);
            let _ = ctx.sink.on_segment(trimmed, start_ms, "auto");
        }
        if let Some(record) = ctx.on_latency {
            record(inference_start.elapsed().as_millis() as u64);
        }
    }

    *samples_before_buffer += buffer.len() as u64;
    buffer.clear();
}

fn samples_to_ms(samples: u64) -> u64 {
    samples.saturating_mul(1_000) / SAMPLE_RATE_HZ
}

fn set_worker_thread_priority() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentThread() -> isize;
            fn SetThreadPriority(thread: isize, priority: i32) -> i32;
        }
        const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        }
    }
    #[cfg(not(windows))]
    {
        // Best-effort on non-Windows platforms.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::transcribe::TranscribeErrorCode;
    use rtrb::RingBuffer;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicU64;

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
        inference_started: Option<Arc<AtomicBool>>,
        block_until: Option<Arc<AtomicBool>>,
    }

    impl Default for MockEngine {
        fn default() -> Self {
            Self {
                segments: Vec::new(),
                loaded: true,
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
        fn transcribe_pcm(&self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
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

    fn ring_pair(capacity: usize) -> (rtrb::Producer<f32>, rtrb::Consumer<f32>) {
        RingBuffer::<f32>::new(capacity)
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
        let (mut prod, cons) = ring_pair(INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        for _ in 0..INFERENCE_WINDOW_SAMPLES {
            prod.push(0.1).expect("push pcm");
        }

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
        let (mut prod, cons) = ring_pair(INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        for _ in 0..INFERENCE_WINDOW_SAMPLES {
            prod.push(0.2).expect("push");
        }

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
        let (mut prod, cons) = ring_pair(INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        for _ in 0..INFERENCE_WINDOW_SAMPLES {
            prod.push(0.3).expect("push");
        }

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
        let (mut prod, cons) = ring_pair(INFERENCE_WINDOW_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        for _ in 0..INFERENCE_WINDOW_SAMPLES {
            prod.push(0.4).expect("push");
        }

        let deadline = Instant::now() + Duration::from_secs(2);
        while !called.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        assert!(called.load(Ordering::SeqCst));
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
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
}
