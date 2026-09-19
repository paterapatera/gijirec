//! Capture session application services (start-only session control).

mod observability;
mod service;

pub use observability::{
    CAPTURE_SESSION_LOG_TARGET, CaptureSessionObservability, NoopCaptureSessionObservability,
    RecordingCaptureSessionObservability,
};
pub use service::{
    CaptureSessionClock, CaptureSessionError, CaptureSessionEvents, CaptureSessionPlatform,
    CaptureSessionProcessingHook, CaptureSessionService, CaptureSessionServiceApi,
    CaptureSessionSnapshot, NoopCaptureSessionEvents, NoopCaptureSessionProcessingHook,
    SystemCaptureSessionClock,
};
