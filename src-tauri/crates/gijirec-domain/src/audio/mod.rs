//! Domain audio types.
pub mod capture_audio_controls;
pub mod device;
pub mod error;
pub mod fixtures;
pub mod pcm_chunk;
pub mod phase;

pub use capture_audio_controls::{
    CaptureAudioControls, DEFAULT_INGEST_GAIN, IngestGainValidationError, MAX_INGEST_GAIN,
    MIN_INGEST_GAIN, validate_manual_ingest_gain,
};
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
