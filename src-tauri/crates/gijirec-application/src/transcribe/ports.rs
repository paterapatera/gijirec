//! Application ports for whisper transcribe orchestration.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gijirec_domain::transcribe::TranscribeError;

/// Progress payload per `docs/contracts/whisper-transcribe-status.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub percent: Option<f64>,
    pub status: ModelDownloadStatus,
}

/// Download lifecycle status emitted through progress callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelDownloadStatus {
    Downloading,
    Verifying,
    Complete,
    Failed,
}

/// Local model path resolution and integrity verification.
pub trait ModelStorePort: Send {
    fn model_path(&self) -> PathBuf;
    fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError>;
}

/// HTTPS model acquisition with streaming progress.
pub trait ModelDownloaderPort: Send {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError>;
}

/// 推論ワーカーの起動・停止。infrastructure の TranscribeWorker が実装。
pub trait TranscribeWorkerPort: Send {
    fn prepare_model_path(&mut self, path: &Path) -> Result<(), TranscribeError>;
    fn spawn(&mut self) -> Result<(), TranscribeError>;
    fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError>;
}

/// whisper コンテキストのロード。infrastructure の WhisperCppAdapter が実装。
pub trait WhisperContextPort: Send {
    fn load_model(&mut self, path: &Path) -> Result<(), TranscribeError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct MockWorker;
    struct MockContext;
    struct MockStore;
    struct MockDownloader;

    impl TranscribeWorkerPort for MockWorker {
        fn prepare_model_path(&mut self, _path: &Path) -> Result<(), TranscribeError> {
            Ok(())
        }

        fn spawn(&mut self) -> Result<(), TranscribeError> {
            Ok(())
        }

        fn stop_and_join(&mut self, _timeout: Duration) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    impl WhisperContextPort for MockContext {
        fn load_model(&mut self, _path: &Path) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    impl ModelStorePort for MockStore {
        fn model_path(&self) -> PathBuf {
            PathBuf::from("/tmp/model.bin")
        }

        fn verify(&self, _expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
            Ok(self.model_path())
        }
    }

    impl ModelDownloaderPort for MockDownloader {
        fn download(
            &self,
            _url: &str,
            _destination: &Path,
            _on_progress: &mut dyn FnMut(ModelDownloadProgress),
        ) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    #[test]
    fn ports_are_object_safe_and_injectable() {
        let mut worker: Box<dyn TranscribeWorkerPort> = Box::new(MockWorker);
        let mut context: Box<dyn WhisperContextPort> = Box::new(MockContext);
        let store: Box<dyn ModelStorePort> = Box::new(MockStore);
        let downloader: Box<dyn ModelDownloaderPort> = Box::new(MockDownloader);

        worker.spawn().expect("spawn should succeed");
        context
            .load_model(Path::new("model.bin"))
            .expect("load_model should succeed");
        worker
            .stop_and_join(Duration::from_secs(1))
            .expect("stop_and_join should succeed");
        store.verify(None).expect("verify should succeed");
        downloader
            .download(
                "https://example.test/model.bin",
                Path::new("model.bin"),
                &mut |_| {},
            )
            .expect("download should succeed");
    }
}
