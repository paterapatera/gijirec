//! Infrastructure crate. Depends on domain (adapters).
pub use gijirec_domain as domain;

pub mod audio;
pub mod transcribe;

pub use transcribe::{
    MODEL_FILENAME, ModelDownloadProgress, ModelDownloadStatus, ModelDownloader, ModelStore,
    TranscribeWorker, WhisperCppAdapter, WhisperSegment,
};
