//! Session-scoped capture audio controls (application layer).

pub mod service;
pub mod store;

pub use service::{
    CaptureAudioControlsApplyPort, CaptureAudioControlsError, CaptureAudioControlsErrorCode,
    CaptureAudioControlsEvents, CaptureAudioControlsPatch, CaptureAudioControlsService,
    CaptureAudioControlsState, CapturePhasePort, DefaultCaptureAudioControlsService,
    IngestSourcePort, NoopCaptureAudioControlsApplyPort, NoopCaptureAudioControlsEvents,
    NoopCapturePhasePort, NoopIngestSourcePort,
};
pub use store::CaptureAudioControlsStore;
