//! Host tracing backend for whisper transcribe observability.

use gijirec_presentation::domain::transcribe::{
    TranscribeError, TranscribeErrorCode, TranscribePhase, WhisperModelVariant,
};
use gijirec_presentation::tauri::observability::session_id;
use gijirec_presentation::transcribe::observability::{
    TRANSCRIBE_LOG_TARGET, TranscribeObservability,
};

/// Emits structured transcribe events via `tracing` without raw audio or text.
pub struct TracingTranscribeObservability;

fn variant_tracing_label(variant: WhisperModelVariant) -> &'static str {
    match variant {
        WhisperModelVariant::Q5_0 => "q5_0",
        WhisperModelVariant::Q8_0 => "q8_0",
        WhisperModelVariant::Fp16 => "fp16",
    }
}

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

    fn log_inference_window_level(
        &self,
        window_rms: f32,
        samples_count: usize,
        inference_skipped: bool,
    ) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_window_rms = window_rms,
            transcribe_window_rms_dbfs = rms_to_dbfs(window_rms),
            transcribe_window_samples = samples_count,
            transcribe_inference_skipped = inference_skipped,
            session_id = session_id(),
            "transcribe inference window level"
        );
    }

    fn log_pcm_ingest_rms_summary(
        &self,
        min_rms: f32,
        max_rms: f32,
        mean_rms: f32,
        chunk_count: u64,
    ) {
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            transcribe_pcm_ingest_min_rms = min_rms,
            transcribe_pcm_ingest_max_rms = max_rms,
            transcribe_pcm_ingest_mean_rms = mean_rms,
            transcribe_pcm_ingest_min_rms_dbfs = rms_to_dbfs(min_rms),
            transcribe_pcm_ingest_max_rms_dbfs = rms_to_dbfs(max_rms),
            transcribe_pcm_ingest_mean_rms_dbfs = rms_to_dbfs(mean_rms),
            transcribe_pcm_ingest_chunk_count = chunk_count,
            session_id = session_id(),
            "transcribe pcm ingest rms summary"
        );
    }

    fn log_model_variant_selected(&self, variant: WhisperModelVariant) {
        let label = variant_tracing_label(variant);
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            model_variant_selected = label,
            transcribe_active_model_variant = label,
            session_id = session_id(),
            "transcribe model variant selected"
        );
    }

    fn log_model_variant_applied(&self, variant: WhisperModelVariant) {
        let label = variant_tracing_label(variant);
        tracing::info!(
            target: TRANSCRIBE_LOG_TARGET,
            model_variant_applied = label,
            transcribe_active_model_variant = label,
            session_id = session_id(),
            "transcribe model variant applied"
        );
    }
}

fn rms_to_dbfs(rms: f32) -> f32 {
    if rms <= 0.0 {
        -120.0
    } else {
        20.0 * rms.log10()
    }
}
