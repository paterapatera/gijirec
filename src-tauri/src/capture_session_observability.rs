//! Host tracing backend for capture session observability.

use gijirec_presentation::application::capture_session::{
    CAPTURE_SESSION_LOG_TARGET, CaptureSessionObservability,
};
use gijirec_presentation::domain::capture_session::CaptureSessionPhase;
use gijirec_presentation::tauri::observability::session_id;

/// Emits structured capture-session events via `tracing` (codes only; no PCM or transcript text).
pub struct TracingCaptureSessionObservability;

impl CaptureSessionObservability for TracingCaptureSessionObservability {
    fn log_session_phase_transition(
        &self,
        _from: CaptureSessionPhase,
        to: CaptureSessionPhase,
        transition_busy: bool,
    ) {
        tracing::info!(
            target: CAPTURE_SESSION_LOG_TARGET,
            session_phase = to.as_str(),
            transition_busy,
            session_id = session_id(),
            "capture session phase transition"
        );
    }
}
