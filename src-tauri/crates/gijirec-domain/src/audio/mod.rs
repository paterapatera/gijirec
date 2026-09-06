//! Domain audio types.
pub mod device;
pub mod error;
pub mod pcm_chunk;
pub mod phase;

pub use device::{
    AudioDeviceId, AudioDeviceIdError, AudioDeviceInfo, AudioDeviceKind, AudioDeviceList,
    DeviceSelection,
};
pub use error::{CaptureError, UserFacingError, UserFacingErrorCode};
pub use pcm_chunk::{
    CHANNELS, CHUNK_FRAME_COUNT, PcmChunk, PcmChunkConsumer, PcmChunkError, PcmConsumerError,
    SAMPLE_RATE_HZ, SampleFormat,
};
pub use phase::{CapturePhase, PhaseTransitionError};
