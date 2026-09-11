//! No-op transcribe ports shared by integration and unit tests.

use std::path::Path;
use std::time::Duration;

use gijirec_domain::transcribe::TranscribeError;

use super::ports::{TranscribeWorkerPort, WhisperContextPort};

pub fn noop_prepare_model_path(_path: &Path) -> Result<(), TranscribeError> {
    Ok(())
}

pub fn noop_worker_spawn() -> Result<(), TranscribeError> {
    Ok(())
}

pub fn noop_worker_stop_and_join(_timeout: Duration) -> Result<(), TranscribeError> {
    Ok(())
}

pub fn noop_whisper_load_model(_path: &Path) -> Result<(), TranscribeError> {
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub struct NoopTranscribeWorkerPort;

impl TranscribeWorkerPort for NoopTranscribeWorkerPort {
    fn prepare_model_path(&mut self, path: &Path) -> Result<(), TranscribeError> {
        noop_prepare_model_path(path)
    }

    fn spawn(&mut self) -> Result<(), TranscribeError> {
        noop_worker_spawn()
    }

    fn stop_and_join(&mut self, _timeout: Duration) -> Result<(), TranscribeError> {
        noop_worker_stop_and_join(_timeout)
    }
}

#[derive(Clone, Copy, Default)]
pub struct NoopWhisperContextPort;

impl WhisperContextPort for NoopWhisperContextPort {
    fn load_model(&mut self, path: &Path) -> Result<(), TranscribeError> {
        noop_whisper_load_model(path)
    }
}
