//! Transcribe observability dispatch (bylaw-safe in presentation).

use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};
use std::sync::{OnceLock, RwLock};

/// Structured transcribe observability hooks.
pub trait TranscribeObservability: Send + Sync {
    fn log_phase_transition(&self, phase: TranscribePhase);
    fn log_pcm_sequence_gaps(&self, from: u64, to: u64);
    fn log_block_buffer_drop(&self, drops_total: u64);
    fn log_inference_latency(&self, latency_ms: u64);
    fn log_transcribe_error(&self, error: &TranscribeError);
    fn log_stall_detected(&self);
    fn log_engine_ready(&self) {}
    fn log_inference_started(&self) {}
    fn log_inference_progress(&self, _percent: i32) {}
}

struct NoopTranscribeObservability;

impl TranscribeObservability for NoopTranscribeObservability {
    fn log_phase_transition(&self, _phase: TranscribePhase) {}
    fn log_pcm_sequence_gaps(&self, _from: u64, _to: u64) {}
    fn log_block_buffer_drop(&self, _drops_total: u64) {}
    fn log_inference_latency(&self, _latency_ms: u64) {}
    fn log_transcribe_error(&self, _error: &TranscribeError) {}
    fn log_stall_detected(&self) {}
    fn log_engine_ready(&self) {}
    fn log_inference_started(&self) {}
    fn log_inference_progress(&self, _percent: i32) {}
}

static TRANSCRIBE_OBSERVABILITY: OnceLock<RwLock<Box<dyn TranscribeObservability>>> =
    OnceLock::new();

fn transcribe_observability() -> &'static RwLock<Box<dyn TranscribeObservability>> {
    TRANSCRIBE_OBSERVABILITY.get_or_init(|| RwLock::new(Box::new(NoopTranscribeObservability)))
}

/// Sets the structured transcribe observability backend.
pub fn set_transcribe_observability(backend: Box<dyn TranscribeObservability>) {
    if let Some(lock) = TRANSCRIBE_OBSERVABILITY.get() {
        *lock.write().expect("lock") = backend;
    } else {
        let _ = TRANSCRIBE_OBSERVABILITY.set(RwLock::new(backend));
    }
}

/// Target name for transcribe host tracing (`RUST_LOG=gijirec_transcribe=debug`).
pub const TRANSCRIBE_LOG_TARGET: &str = "gijirec_transcribe";

/// Logs a transcribe phase transition.
pub fn log_phase_transition(phase: TranscribePhase) {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_phase_transition(phase);
}

/// Logs sequence gaps detected on PCM ingestion.
pub fn log_pcm_sequence_gaps(from: u64, to: u64) {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_pcm_sequence_gaps(from, to);
}

/// Logs block buffer overflow drops.
pub fn log_block_buffer_drop(drops_total: u64) {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_block_buffer_drop(drops_total);
}

/// Logs inference latency in milliseconds.
pub fn log_inference_latency(latency_ms: u64) {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_inference_latency(latency_ms);
}

/// Logs transcribe errors without raw PCM or text data.
pub fn log_transcribe_error(error: &TranscribeError) {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_transcribe_error(error);
}

/// Logs a one-shot stall detection diagnostic (`transcribe_stall_detected=true`).
pub fn log_stall_detected() {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_stall_detected();
}

/// Logs that the worker finished loading the whisper engine.
pub fn log_engine_ready() {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_engine_ready();
}

/// Logs that whisper.cpp started an inference window.
pub fn log_inference_started() {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_inference_started();
}

/// Logs whisper.cpp encoder/decoder progress percent (0–100).
pub fn log_inference_progress(percent: i32) {
    transcribe_observability()
        .read()
        .expect("lock")
        .log_inference_progress(percent);
}

/// Restores the noop backend between tests that replace the global observability hook.
#[cfg(test)]
pub fn reset_transcribe_observability_for_tests() {
    set_transcribe_observability(Box::new(NoopTranscribeObservability));
}

/// Records transcribe observability events for unit tests.
#[derive(Clone, Default)]
pub struct RecordingTranscribeObservability {
    pub phases: std::sync::Arc<std::sync::Mutex<Vec<TranscribePhase>>>,
    pub errors: std::sync::Arc<std::sync::Mutex<Vec<TranscribeError>>>,
    pub stall_detected_count: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl RecordingTranscribeObservability {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TranscribeObservability for RecordingTranscribeObservability {
    fn log_phase_transition(&self, phase: TranscribePhase) {
        self.phases.lock().expect("lock").push(phase);
    }

    fn log_pcm_sequence_gaps(&self, _from: u64, _to: u64) {}

    fn log_block_buffer_drop(&self, _drops_total: u64) {}

    fn log_inference_latency(&self, _latency_ms: u64) {}

    fn log_transcribe_error(&self, error: &TranscribeError) {
        self.errors.lock().expect("lock").push(error.clone());
    }

    fn log_stall_detected(&self) {
        self.stall_detected_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Serializes tests that touch the process-wide transcribe observability backend.
#[cfg(test)]
static TEST_OBSERVABILITY_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Runs `f` under the test mutex with a noop backend so parallel tests cannot
/// pollute recorders installed by [`with_test_transcribe_observability`].
#[cfg(test)]
pub fn with_isolated_transcribe_observability<F: FnOnce()>(f: F) {
    let _guard = TEST_OBSERVABILITY_MUTEX
        .lock()
        .expect("lock test observability mutex");
    reset_transcribe_observability_for_tests();
    f();
    reset_transcribe_observability_for_tests();
}

/// Serializes tests that replace the process-wide transcribe observability backend.
#[cfg(test)]
pub fn with_test_transcribe_observability<F: FnOnce()>(
    recorder: &RecordingTranscribeObservability,
    f: F,
) {
    let _guard = TEST_OBSERVABILITY_MUTEX
        .lock()
        .expect("lock test observability mutex");
    reset_transcribe_observability_for_tests();
    set_transcribe_observability(Box::new(recorder.clone()));
    f();
    reset_transcribe_observability_for_tests();
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::transcribe::TranscribeErrorCode;

    #[test]
    fn dispatch_forwards_phase_error_and_stall_to_backend() {
        let recorder = RecordingTranscribeObservability::new();
        with_test_transcribe_observability(&recorder, || {
            log_phase_transition(TranscribePhase::LoadingModel);
            log_phase_transition(TranscribePhase::Ready);
            log_transcribe_error(&TranscribeError::InferenceFailed {
                detail: "stall".to_string(),
            });
            log_stall_detected();
        });

        assert_eq!(
            *recorder.phases.lock().expect("lock"),
            vec![TranscribePhase::LoadingModel, TranscribePhase::Ready]
        );
        let errors = recorder.errors.lock().expect("lock");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].to_user_facing().code,
            TranscribeErrorCode::InferenceFailed
        );
        assert_eq!(
            recorder
                .stall_detected_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}
