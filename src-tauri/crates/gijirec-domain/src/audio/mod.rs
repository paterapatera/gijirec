//! Domain audio types.
pub mod error;
pub mod pcm_chunk;
pub mod phase;

pub use error::{CaptureError, UserFacingError, UserFacingErrorCode};
pub use pcm_chunk::{
    CHANNELS, CHUNK_FRAME_COUNT, PcmChunk, PcmChunkConsumer, PcmChunkError, PcmConsumerError,
    SAMPLE_RATE_HZ, SampleFormat,
};
pub use phase::{CapturePhase, PhaseTransitionError};
