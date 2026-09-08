//! Dedicated-thread PCM consumer and VAD-driven whisper inference worker.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use gijirec_domain::transcribe::{TranscribeError, TranscriptSegmentSink};

use super::whisper_adapter::{WhisperCppAdapter, WhisperSegment};

/// Samples per endpointing frame (100 ms @ 16 kHz, same length as an upstream PCM chunk).
const FRAME_SAMPLES: usize = 1_600;

/// Longest PCM handed to whisper.cpp in one inference (10 s @ 16 kHz).
const MAX_INFERENCE_WINDOW_SAMPLES: usize = 160_000;

/// Maximum samples retained while waiting for the next inference window (20 s @ 16 kHz).
/// Two full windows so that a worker that fell behind can skip to the latest window.
pub const MAX_PCM_BUFFER_SAMPLES: usize = MAX_INFERENCE_WINDOW_SAMPLES * 2;

/// Speech required before a short trailing pause closes the utterance (1 s @ 16 kHz).
const MIN_SPEECH_SAMPLES: usize = 16_000;

/// Trailing near-silence that closes an utterance once enough speech accumulated (300 ms).
const TRAILING_SILENCE_FRAMES: usize = 3;

/// Longer near-silence that closes an utterance regardless of its length (1.2 s).
const LONG_SILENCE_FRAMES: usize = 12;

/// When forced to cut at the max window, search the last 2 s for the quietest frame.
const FORCED_CUT_SEARCH_SAMPLES: usize = 32_000;

/// Smallest unfinished tail still transcribed when the worker shuts down (500 ms).
const MIN_FLUSH_SAMPLES: usize = 8_000;

const SAMPLE_RATE_HZ: u64 = 16_000;

/// Skip whisper.cpp when the window RMS is below this (near-silence).
const SILENCE_RMS_THRESHOLD: f32 = 0.008;

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
}

impl ModelPathLoadable for WhisperCppAdapter {
    fn load_from_path_if_needed(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        if self.is_loaded() {
            return Ok(());
        }
        self.load_model(path)
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

/// VAD-driven inference worker reading PCM from an rtrb consumer.
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

    while running.load(Ordering::SeqCst) {
        let window = take_inference_window(&pcm_buffer, false);
        if let Some((pcm, base_samples)) = window {
            if engine.is_loaded() {
                let ctx = InferenceContext {
                    engine: &mut engine,
                    sink: &sink,
                    on_latency: on_latency.as_ref(),
                    on_attempted: on_inference_attempted.as_ref(),
                };
                run_inference_window(&pcm, base_samples, ctx);
            }
        } else {
            thread::sleep(Duration::from_millis(5));
        }
    }

    if let Some(handle) = drain_handle {
        let _ = handle.join();
    }

    while let Some((pcm, base_samples)) = take_inference_window(&pcm_buffer, true) {
        if engine.is_loaded() {
            let ctx = InferenceContext {
                engine: &mut engine,
                sink: &sink,
                on_latency: on_latency.as_ref(),
                on_attempted: on_inference_attempted.as_ref(),
            };
            run_inference_window(&pcm, base_samples, ctx);
        }
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

/// Cuts the next utterance off the buffer, or `None` when more audio is needed.
///
/// Endpointing runs on 100 ms frames: leading near-silence is discarded, the window
/// ends at the first pause long enough to mark an utterance boundary, and a window
/// that reaches [`MAX_INFERENCE_WINDOW_SAMPLES`] is cut at the quietest frame of its
/// last 2 s. With `flush`, a short unfinished tail is returned as well (shutdown).
fn take_inference_window(
    pcm_buffer: &Arc<Mutex<PcmBufferState>>,
    flush: bool,
) -> Option<(Vec<f32>, u64)> {
    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    take_window_from_state(&mut state, flush)
}

fn take_window_from_state(state: &mut PcmBufferState, flush: bool) -> Option<(Vec<f32>, u64)> {
    trim_leading_silence(state);

    // Live transcription: when a full window has piled up behind the one being cut,
    // skip ahead so the transcript follows the latest audio.
    if state.samples.len() >= MAX_INFERENCE_WINDOW_SAMPLES * 2 {
        let overflow = state.samples.len() - MAX_INFERENCE_WINDOW_SAMPLES;
        let _ = state.samples.drain(..overflow);
        state.samples_before_buffer += overflow as u64;
        trim_leading_silence(state);
    }

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
        if state.samples.len() >= MAX_PCM_BUFFER_SAMPLES {
            state.samples.pop_front();
            state.samples_before_buffer += 1;
        }
        state.samples.push_back(sample);
        popped += 1;
    }
    popped
}

fn window_rms(pcm: &[f32]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = pcm.iter().map(|sample| sample * sample).sum();
    (sum_sq / pcm.len() as f32).sqrt()
}

fn run_inference_window<E: SegmentEngine>(
    pcm: &[f32],
    samples_before_buffer: u64,
    ctx: InferenceContext<'_, E>,
) {
    if pcm.is_empty() {
        return;
    }

    if window_rms(pcm) < SILENCE_RMS_THRESHOLD {
        return;
    }

    if let Some(attempted) = ctx.on_attempted {
        attempted();
    }

    let inference_start = Instant::now();
    let base_ms = samples_to_ms(samples_before_buffer);

    match ctx.engine.transcribe_pcm(pcm) {
        Ok(segments) => {
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
        Err(_) => {
            if let Some(record) = ctx.on_latency {
                record(inference_start.elapsed().as_millis() as u64);
            }
        }
    }
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
            _path: &std::path::Path,
        ) -> Result<(), TranscribeError> {
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
    fn cuts_utterance_at_trailing_silence() {
        let mut state = state_with(&concat(&[tone(0.2, 32_000), silence(9_600)]));

        let (pcm, base) = take_window_from_state(&mut state, false).expect("window");

        assert_eq!(base, 0);
        assert_eq!(
            pcm.len(),
            32_000 + TRAILING_SILENCE_FRAMES * FRAME_SAMPLES,
            "window must end right after the closing pause"
        );
        assert_eq!(
            state.samples.len(),
            9_600 - TRAILING_SILENCE_FRAMES * FRAME_SAMPLES,
            "extra silence stays for the next window"
        );
        assert_eq!(state.samples_before_buffer, pcm.len() as u64);
    }

    #[test]
    fn drops_leading_silence_and_offsets_base_timestamp() {
        let mut state = state_with(&concat(&[
            silence(16_000),
            tone(0.2, 32_000),
            silence(9_600),
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
        let mut state = state_with(&concat(&[tone(0.2, 8_000), silence(9_600)]));
        assert!(
            take_window_from_state(&mut state, false).is_none(),
            "a brief pause after short speech must not close the utterance"
        );

        state
            .samples
            .extend(silence(LONG_SILENCE_FRAMES * FRAME_SAMPLES - 9_600));
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
        // A dip that is audible (above the silence threshold) ending at 9.0 s.
        let dip_end = 144_000;
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
    fn skips_to_latest_window_when_two_windows_behind() {
        let mut state = state_with(&concat(&[
            tone(0.4, MAX_INFERENCE_WINDOW_SAMPLES),
            tone(0.8, MAX_INFERENCE_WINDOW_SAMPLES),
        ]));

        let (window, base) = take_window_from_state(&mut state, false).expect("window");

        assert_eq!(base, MAX_INFERENCE_WINDOW_SAMPLES as u64);
        assert!((window[0] - 0.8).abs() < f32::EPSILON);
        assert_eq!(window.len(), MAX_INFERENCE_WINDOW_SAMPLES);
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
        let (mut prod, cons) = ring_pair(UTTERANCE_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_utterance(&mut prod, 0.1);

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
        let (mut prod, cons) = ring_pair(UTTERANCE_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_utterance(&mut prod, 0.2);

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
        let (mut prod, cons) = ring_pair(UTTERANCE_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_utterance(&mut prod, 0.3);

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
        let (mut prod, cons) = ring_pair(UTTERANCE_SAMPLES + 1_024);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_utterance(&mut prod, 0.4);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !called.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        assert!(called.load(Ordering::SeqCst));
        worker.stop_and_join(Duration::from_secs(1)).expect("stop");
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
        let (mut prod, cons) = ring_pair(MAX_PCM_BUFFER_SAMPLES * 2 + MAX_INFERENCE_WINDOW_SAMPLES);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        let pump = thread::spawn(move || {
            let mut pushed = 0usize;
            for _ in 0..MAX_PCM_BUFFER_SAMPLES + MAX_INFERENCE_WINDOW_SAMPLES {
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
    fn transcribes_latest_window_when_inference_falls_behind() {
        let (sink, segments) = recording_sink();
        let inference_started = Arc::new(AtomicBool::new(false));
        let unblock = Arc::new(AtomicBool::new(false));
        let last_first_sample = Arc::new(Mutex::new(None::<f32>));
        let last_first_sample_capture = Arc::clone(&last_first_sample);

        struct LatestWindowEngine {
            inner: MockEngine,
            last_first_sample: Arc<Mutex<Option<f32>>>,
        }

        impl SegmentEngine for LatestWindowEngine {
            fn transcribe_pcm(
                &mut self,
                pcm: &[f32],
            ) -> Result<Vec<WhisperSegment>, TranscribeError> {
                *self.last_first_sample.lock().expect("lock") = pcm.first().copied();
                self.inner.transcribe_pcm(pcm)
            }

            fn is_loaded(&self) -> bool {
                self.inner.is_loaded()
            }
        }

        impl ModelPathLoadable for LatestWindowEngine {
            fn load_from_path_if_needed(
                &mut self,
                path: &std::path::Path,
            ) -> Result<(), TranscribeError> {
                self.inner.load_from_path_if_needed(path)
            }
        }

        let engine = LatestWindowEngine {
            inner: MockEngine {
                segments: vec![WhisperSegment {
                    text: "latest".to_string(),
                    start_ms: 0,
                    end_ms: 100,
                }],
                inference_started: Some(Arc::clone(&inference_started)),
                block_until: Some(Arc::clone(&unblock)),
                ..MockEngine::default()
            },
            last_first_sample: last_first_sample_capture,
        };
        let mut worker = TranscribeWorker::with_engine(sink, engine);
        let (mut prod, cons) = ring_pair(MAX_INFERENCE_WINDOW_SAMPLES * 3 + UTTERANCE_SAMPLES);
        worker.attach_pcm_consumer(cons);
        worker.spawn().expect("spawn");

        push_utterance(&mut prod, 0.1);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !inference_started.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(inference_started.load(Ordering::SeqCst));

        // Two full windows arrive while inference is blocked: the stale one must be skipped.
        push_samples(&mut prod, 0.4, MAX_INFERENCE_WINDOW_SAMPLES);
        push_samples(&mut prod, 0.8, MAX_INFERENCE_WINDOW_SAMPLES);

        thread::sleep(Duration::from_millis(200));
        unblock.store(true, Ordering::SeqCst);

        let deadline = Instant::now() + Duration::from_secs(2);
        while segments.lock().expect("lock").len() < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }

        let recorded = segments.lock().expect("lock").clone();
        assert!(
            recorded.len() >= 2,
            "expected catch-up inference on latest audio"
        );
        let first_sample = last_first_sample.lock().expect("lock").unwrap_or(0.0);
        assert!(
            (first_sample - 0.8).abs() < f32::EPSILON,
            "behind worker must transcribe the latest window, got first sample {first_sample}"
        );

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
}
