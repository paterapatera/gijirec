//! Application ports for whisper transcribe orchestration.

use std::path::{Path, PathBuf};
use std::time::Duration;

pub use gijirec_domain::transcribe::{ModelDownloadProgress, ModelDownloadStatus};
use gijirec_domain::transcribe::{TranscribeError, WhisperModelVariant};

/// Local model path resolution and integrity verification.
pub trait ModelStorePort: Send {
    fn model_path(&self) -> PathBuf;
    fn model_path_for(&self, variant: WhisperModelVariant) -> PathBuf;
    fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError>;
    fn verify_variant(
        &self,
        variant: WhisperModelVariant,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf, TranscribeError>;
    fn file_exists(&self, variant: WhisperModelVariant) -> bool;
}

/// HTTPS model acquisition with streaming progress.
pub trait ModelDownloaderPort: Send {
    /* jscpd:ignore-start */
    fn download(
        &self,
        url: &str,
        destination: &Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError>;
    /* jscpd:ignore-end */
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
    use crate::transcribe::noop_ports::{NoopTranscribeWorkerPort, NoopWhisperContextPort};
    use crate::transcribe::test_support::NoopModelDownloader;
    use std::path::PathBuf;

    struct MockStore;

    impl ModelStorePort for MockStore {
        fn model_path(&self) -> PathBuf {
            PathBuf::from("/tmp/model.bin")
        }

        fn model_path_for(&self, _variant: WhisperModelVariant) -> PathBuf {
            self.model_path()
        }

        fn verify(&self, _expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
            Ok(self.model_path())
        }

        fn verify_variant(
            &self,
            _variant: WhisperModelVariant,
            _expected_sha256: Option<&str>,
        ) -> Result<PathBuf, TranscribeError> {
            Ok(self.model_path())
        }

        fn file_exists(&self, _variant: WhisperModelVariant) -> bool {
            true
        }
    }

    #[test]
    fn ports_are_object_safe_and_injectable() {
        let mut worker: Box<dyn TranscribeWorkerPort> = Box::new(NoopTranscribeWorkerPort);
        let mut context: Box<dyn WhisperContextPort> = Box::new(NoopWhisperContextPort);
        let store: Box<dyn ModelStorePort> = Box::new(MockStore);
        let downloader: Box<dyn ModelDownloaderPort> = Box::new(NoopModelDownloader);

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
