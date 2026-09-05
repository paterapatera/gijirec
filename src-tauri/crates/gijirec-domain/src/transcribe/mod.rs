//! Whisper transcribe domain types and contracts.
pub mod error;
pub mod phase;
pub mod segment_sink;
pub mod transcript_block;

pub use error::{TranscribeError, TranscribeErrorCode, UserFacingTranscribeError};
pub use phase::{PhaseTransitionError, TranscribePhase};
pub use segment_sink::TranscriptSegmentSink;
pub use transcript_block::{
    TranscriptBlock, TranscriptBlockConsumer, TranscriptBlockError, TranscriptConsumerError,
};
