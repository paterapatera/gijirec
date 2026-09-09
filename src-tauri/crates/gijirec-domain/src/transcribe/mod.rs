//! Whisper transcribe domain types and contracts.
pub mod error;
pub mod model_variant;
pub mod phase;
pub mod segment_sink;
pub mod settings;
pub mod settings_error;
pub mod transcript_block;

pub use error::{TranscribeError, TranscribeErrorCode, UserFacingTranscribeError};
pub use model_variant::{ModelVariantCatalog, ModelVariantDescriptor, WhisperModelVariant};
pub use phase::{PhaseTransitionError, TranscribePhase};
pub use segment_sink::TranscriptSegmentSink;
pub use settings::TranscribeSettings;
pub use settings_error::{
    TranscribeSettingsError, TranscribeSettingsErrorCode, TranscribeSettingsLoadIssue,
    TranscribeSettingsLoadResult, TranscribeSettingsUserError,
};
pub use transcript_block::{
    TranscriptBlock, TranscriptBlockConsumer, TranscriptBlockError, TranscriptConsumerError,
};
