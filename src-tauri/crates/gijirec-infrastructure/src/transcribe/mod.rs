//! Whisper transcribe infrastructure adapters.
mod model_downloader;
mod model_store;
mod transcribe_worker;
mod whisper_adapter;

pub use model_downloader::{ModelDownloadProgress, ModelDownloadStatus, ModelDownloader};
pub use model_store::{MODEL_FILENAME, ModelStore};
pub use transcribe_worker::{
    BatchCycleCompleted, BatchCycleStarted, InferenceWindowLevel, MAX_PCM_BUFFER_SAMPLES,
    ModelPathLoadable, SegmentEngine, TranscribeWorker,
};
pub use whisper_adapter::{WhisperCppAdapter, WhisperSegment};
