//! Whisper transcribe infrastructure adapters.
mod model_downloader;
mod model_store;
mod test_macros;
mod transcribe_worker;
mod whisper_adapter;

#[cfg(test)]
mod test_temp;

pub use gijirec_domain::transcribe::{ModelDownloadProgress, ModelDownloadStatus};
pub use model_downloader::ModelDownloader;
pub use model_store::{MODEL_FILENAME, ModelStore};
pub use transcribe_worker::{
    BatchCycleCompleted, BatchCycleStarted, InferenceWindowLevel, MAX_PCM_BUFFER_SAMPLES,
    ModelPathLoadable, SegmentEngine, TranscribeWorker,
};
pub use whisper_adapter::{WhisperCppAdapter, WhisperSegment};
