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
}

struct NoopTranscribeObservability;

impl TranscribeObservability for NoopTranscribeObservability {
    fn log_phase_transition(&self, _phase: TranscribePhase) {}
    fn log_pcm_sequence_gaps(&self, _from: u64, _to: u64) {}
    fn log_block_buffer_drop(&self, _drops_total: u64) {}
    fn log_inference_latency(&self, _latency_ms: u64) {}
    fn log_transcribe_error(&self, _error: &TranscribeError) {}
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
