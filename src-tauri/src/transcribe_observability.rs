//! Host tracing backend for whisper transcribe observability.

use gijirec_presentation::domain::transcribe::{
    TranscribeError, TranscribeErrorCode, TranscribePhase,
};
use gijirec_presentation::tauri::observability::session_id;
use gijirec_presentation::transcribe::observability::{
    TRANSCRIBE_LOG_TARGET, TranscribeObservability,
};

/// Emits structured transcribe events via `tracing` without raw audio or text.
pub struct TracingTranscribeObservability;

impl TranscribeObservability for TracingTranscribeObservability {
    fn log_phase_transition(&self, phase: TranscribePhase) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_phase = phase.as_str(),
            session_id = session_id(),
            "transcribe phase transition"
        );
    }

    fn log_pcm_sequence_gaps(&self, from: u64, to: u64) {
        tracing::warn!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_pcm_sequence_gaps = true,
            gap_from = from,
            gap_to = to,
            session_id = session_id(),
            "pcm sequence gap detected on ingest"
        );
    }

    fn log_block_buffer_drop(&self, drops_total: u64) {
        tracing::warn!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_block_buffer_drops = drops_total,
            session_id = session_id(),
            "transcript block bus dropped oldest block due to queue capacity overflow"
        );
    }

    fn log_inference_latency(&self, latency_ms: u64) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_inference_latency_ms = latency_ms,
            session_id = session_id(),
            "transcribe inference completed"
        );
    }

    fn log_transcribe_error(&self, error: &TranscribeError) {
        let code = error.to_user_facing().code;
        tracing::error!(
            target: TRANSCRIBE_LOG_TARGET,
            error_code = code.as_str(),
            session_id = session_id(),
            "transcribe error occurred"
        );
    }

    fn log_stall_detected(&self) {
        tracing::warn!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_stall_detected = true,
            error_code = TranscribeErrorCode::InferenceFailed.as_str(),
            session_id = session_id(),
            "transcription stall detected"
        );
    }

    fn log_engine_ready(&self) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_engine_ready = true,
            session_id = session_id(),
            "whisper engine ready on worker thread"
        );
    }

    fn log_inference_started(&self) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_inference_started = true,
            session_id = session_id(),
            "transcribe inference started"
        );
    }

    fn log_inference_progress(&self, percent: i32) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_inference_progress_pct = percent,
            session_id = session_id(),
            "transcribe inference progress"
        );
    }

    fn log_batch_cycle_started(
        &self,
        cycle_id: u64,
        samples_count: usize,
        pcm_backlog_seconds: f64,
        rtrb_overflow_count: u64,
    ) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            batch_cycle_started = true,
            cycle_id = cycle_id,
            samples_count = samples_count,
            transcribe_pcm_backlog_seconds = pcm_backlog_seconds,
            transcribe_rtrb_overflow_count = rtrb_overflow_count,
            session_id = session_id(),
            "transcribe batch cycle started"
        );
    }

    fn log_batch_cycle_completed(
        &self,
        cycle_id: u64,
        duration_ms: u64,
        samples_count: usize,
        segments_count: usize,
    ) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            batch_cycle_completed = true,
            cycle_id = cycle_id,
            transcribe_batch_duration_ms = duration_ms,
            samples_count = samples_count,
            segments_count = segments_count,
            session_id = session_id(),
            "transcribe batch cycle completed"
        );
    }
}
