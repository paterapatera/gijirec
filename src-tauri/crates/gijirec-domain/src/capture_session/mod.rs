//! User capture session domain types per `docs/contracts/capture-session-toggle.md`.
pub mod error;
pub mod phase;

pub use error::CaptureSessionErrorCode;
pub use phase::CaptureSessionPhase;
