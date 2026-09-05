//! Host tracing backend for capture observability.

use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
use gijirec_presentation::tauri::observability::{CAPTURE_LOG_TARGET, CaptureObservability};

/// Emits structured capture events via `tracing` (host-only; no PCM or device names).
pub struct TracingCaptureObservability;

impl CaptureObservability for TracingCaptureObservability {
    fn log_phase_transition(&self, phase: CapturePhase) {
        tracing::info!(
            target: CAPTURE_LOG_TARGET,
            capture_phase = phase.as_str(),
            "capture phase transition"
        );
    }

    fn log_buffer_drop(&self, drops_total: u64) {
        tracing::warn!(
            target: CAPTURE_LOG_TARGET,
            capture_buffer_drops_total = drops_total,
            "pcm chunk bus dropped oldest queued chunk"
        );
    }

    fn log_stream_open_failure(&self, port: &str, error: &CaptureError, correlation_id: &str) {
        let code = error.clone().to_user_facing().code;
        tracing::error!(
            target: CAPTURE_LOG_TARGET,
            port,
            error_code = code.as_str(),
            correlation_id,
            "capture stream open failed"
        );
    }

    fn log_rt_callback_max_us(&self, max_us: u64) {
        tracing::info!(
            target: CAPTURE_LOG_TARGET,
            capture_rt_callback_max_us = max_us,
            "capture rt callback max observed"
        );
    }
}
