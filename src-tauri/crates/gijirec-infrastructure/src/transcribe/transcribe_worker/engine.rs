use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use gijirec_domain::transcribe::TranscribeError;

use crate::transcribe::whisper_adapter::{WhisperCppAdapter, WhisperSegment};

/// Inference engine abstraction (production: [`WhisperCppAdapter`], tests: mocks).
pub trait SegmentEngine: Send {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError>;
    fn is_loaded(&self) -> bool;
    fn set_progress_hook(&mut self, _hook: Arc<dyn Fn(i32) + Send + Sync>) {}
    fn set_running_flag(&mut self, _running: Arc<AtomicBool>) {}
}

/// Loads a whisper model path on the worker thread before inference begins.
pub trait ModelPathLoadable: SegmentEngine {
    fn load_from_path_if_needed(&mut self, path: &std::path::Path) -> Result<(), TranscribeError>;
    fn reload_from_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        self.load_from_path_if_needed(path)
    }
}

impl ModelPathLoadable for WhisperCppAdapter {
    fn load_from_path_if_needed(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        if self.is_loaded() {
            return Ok(());
        }
        self.load_model(path)
    }

    fn reload_from_path(&mut self, path: &std::path::Path) -> Result<(), TranscribeError> {
        self.reload_model(path)
    }
}

impl SegmentEngine for WhisperCppAdapter {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        WhisperCppAdapter::transcribe_pcm(self, pcm)
    }

    fn is_loaded(&self) -> bool {
        WhisperCppAdapter::is_loaded(self)
    }

    fn set_progress_hook(&mut self, hook: Arc<dyn Fn(i32) + Send + Sync>) {
        WhisperCppAdapter::set_progress_hook(self, hook);
    }
}
