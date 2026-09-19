//! Capture session observability hooks (no tracing in application layer).

use std::sync::{Arc, Mutex};

use gijirec_domain::capture_session::CaptureSessionPhase;

/// Target name for host tracing (`RUST_LOG=gijirec_capture_session=debug`).
pub const CAPTURE_SESSION_LOG_TARGET: &str = "gijirec_capture_session";

/// Structured capture-session observability (host implements with `tracing`).
pub trait CaptureSessionObservability: Send + Sync {
    fn log_session_phase_transition(
        &self,
        from: CaptureSessionPhase,
        to: CaptureSessionPhase,
        transition_busy: bool,
    );
}

/// No-op backend for tests and default wiring.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCaptureSessionObservability;

impl CaptureSessionObservability for NoopCaptureSessionObservability {
    fn log_session_phase_transition(
        &self,
        _from: CaptureSessionPhase,
        _to: CaptureSessionPhase,
        _transition_busy: bool,
    ) {
    }
}

/// In-memory recorder for unit tests.
#[derive(Clone, Default)]
pub struct RecordingCaptureSessionObservability {
    pub phase_transitions: Arc<Mutex<Vec<(CaptureSessionPhase, CaptureSessionPhase)>>>,
    pub transition_busy_flags: Arc<Mutex<Vec<bool>>>,
}

impl RecordingCaptureSessionObservability {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CaptureSessionObservability for RecordingCaptureSessionObservability {
    fn log_session_phase_transition(
        &self,
        from: CaptureSessionPhase,
        to: CaptureSessionPhase,
        transition_busy: bool,
    ) {
        self.phase_transitions
            .lock()
            .expect("lock")
            .push((from, to));
        self.transition_busy_flags
            .lock()
            .expect("lock")
            .push(transition_busy);
    }
}
