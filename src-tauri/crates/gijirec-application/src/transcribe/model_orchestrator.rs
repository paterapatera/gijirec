//! Model existence verification and download orchestration.

use std::path::PathBuf;

use gijirec_domain::transcribe::TranscribeError;

use super::ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
};

/// Configuration for whisper model acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOrchestratorConfig {
    pub model_url: String,
    pub expected_sha256: String,
}

/// Orchestrates local model verification and download when required.
pub struct ModelOrchestrator<S, D> {
    store: S,
    downloader: D,
    config: ModelOrchestratorConfig,
}

impl<S, D> ModelOrchestrator<S, D> {
    pub fn new(store: S, downloader: D, config: ModelOrchestratorConfig) -> Self {
        Self {
            store,
            downloader,
            config,
        }
    }
}

impl<S: ModelStorePort, D: ModelDownloaderPort> ModelOrchestrator<S, D> {
    /// Ensures a valid local model exists, downloading when missing or corrupt.
    pub fn ensure_model<F>(&self, mut on_progress: F) -> Result<PathBuf, TranscribeError>
    where
        F: FnMut(ModelDownloadProgress),
    {
        if let Ok(path) = self.store.verify(Some(&self.config.expected_sha256)) {
            return Ok(path);
        }

        if self.config.model_url.is_empty() {
            return Err(TranscribeError::ModelNotFound {
                detail: "model file not found and download URL is not configured".to_string(),
            });
        }

        let destination = self.store.model_path();
        self.downloader
            .download(&self.config.model_url, &destination, &mut on_progress)?;

        emit_progress(&mut on_progress, ModelDownloadStatus::Verifying);
        self.store.verify(Some(&self.config.expected_sha256))
    }
}

fn emit_progress<F>(on_progress: &mut F, status: ModelDownloadStatus)
where
    F: FnMut(ModelDownloadProgress),
{
    on_progress(ModelDownloadProgress {
        bytes_downloaded: 0,
        bytes_total: None,
        percent: None,
        status,
    });
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use gijirec_domain::transcribe::TranscribeErrorCode;

    use super::*;
    use crate::transcribe::ports::ModelDownloadProgress;

    const EXPECTED_SHA: &str = "abc123";
    const MODEL_URL: &str = "https://example.test/model.bin";

    struct MockStore {
        model_path: PathBuf,
        verify_results: Mutex<Vec<Result<PathBuf, TranscribeError>>>,
        verify_calls: AtomicUsize,
    }

    impl MockStore {
        fn new(
            model_path: PathBuf,
            verify_results: Vec<Result<PathBuf, TranscribeError>>,
        ) -> Arc<Self> {
            Arc::new(Self {
                model_path,
                verify_results: Mutex::new(verify_results),
                verify_calls: AtomicUsize::new(0),
            })
        }

        fn verify_call_count(self: &Arc<Self>) -> usize {
            self.verify_calls.load(Ordering::SeqCst)
        }
    }

    impl ModelStorePort for Arc<MockStore> {
        fn model_path(&self) -> PathBuf {
            self.model_path.clone()
        }

        fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
            self.verify_calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(expected_sha256, Some(EXPECTED_SHA));

            let mut results = self.verify_results.lock().expect("lock verify results");
            if results.is_empty() {
                panic!("unexpected verify call");
            }
            results.remove(0)
        }
    }

    struct MockDownloader {
        download_calls: AtomicUsize,
        result: Mutex<Result<(), TranscribeError>>,
    }

    impl MockDownloader {
        fn success() -> Arc<Self> {
            Arc::new(Self {
                download_calls: AtomicUsize::new(0),
                result: Mutex::new(Ok(())),
            })
        }

        fn failure(err: TranscribeError) -> Arc<Self> {
            Arc::new(Self {
                download_calls: AtomicUsize::new(0),
                result: Mutex::new(Err(err)),
            })
        }

        fn download_call_count(self: &Arc<Self>) -> usize {
            self.download_calls.load(Ordering::SeqCst)
        }
    }

    impl ModelDownloaderPort for Arc<MockDownloader> {
        fn download(
            &self,
            url: &str,
            destination: &Path,
            on_progress: &mut dyn FnMut(ModelDownloadProgress),
        ) -> Result<(), TranscribeError> {
            self.download_calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(url, MODEL_URL);
            assert_eq!(destination, Path::new("/tmp/models/model.bin"));

            on_progress(ModelDownloadProgress {
                bytes_downloaded: 100,
                bytes_total: Some(100),
                percent: Some(100.0),
                status: ModelDownloadStatus::Downloading,
            });

            let result = self.result.lock().expect("lock download result").clone();
            if result.is_ok() {
                on_progress(ModelDownloadProgress {
                    bytes_downloaded: 100,
                    bytes_total: Some(100),
                    percent: Some(100.0),
                    status: ModelDownloadStatus::Complete,
                });
            }
            result
        }
    }

    fn config() -> ModelOrchestratorConfig {
        ModelOrchestratorConfig {
            model_url: MODEL_URL.to_string(),
            expected_sha256: EXPECTED_SHA.to_string(),
        }
    }

    fn orchestrator(
        store: Arc<MockStore>,
        downloader: Arc<MockDownloader>,
    ) -> ModelOrchestrator<Arc<MockStore>, Arc<MockDownloader>> {
        ModelOrchestrator::new(store, downloader, config())
    }

    #[test]
    fn existing_valid_model_skips_download() {
        let model_path = PathBuf::from("/tmp/models/model.bin");
        let store = MockStore::new(model_path.clone(), vec![Ok(model_path.clone())]);
        let downloader = MockDownloader::success();

        let path = orchestrator(Arc::clone(&store), Arc::clone(&downloader))
            .ensure_model(|_| {})
            .expect("valid model should succeed");

        assert_eq!(path, model_path);
        assert_eq!(store.verify_call_count(), 1);
        assert_eq!(downloader.download_call_count(), 0);
    }

    #[test]
    fn missing_model_triggers_download_and_progress_callback() {
        let model_path = PathBuf::from("/tmp/models/model.bin");
        let store = MockStore::new(
            model_path.clone(),
            vec![
                Err(TranscribeError::ModelNotFound {
                    detail: "missing".to_string(),
                }),
                Ok(model_path.clone()),
            ],
        );
        let downloader = MockDownloader::success();
        let progress = Arc::new(Mutex::new(Vec::new()));

        let path = orchestrator(Arc::clone(&store), Arc::clone(&downloader))
            .ensure_model(|update| progress.lock().expect("lock").push(update))
            .expect("download should succeed");

        assert_eq!(path, model_path);
        assert_eq!(store.verify_call_count(), 2);
        assert_eq!(downloader.download_call_count(), 1);

        let updates = progress.lock().expect("lock");
        assert!(
            updates
                .iter()
                .any(|p| p.status == ModelDownloadStatus::Downloading),
            "must emit downloading progress"
        );
        assert!(
            updates
                .iter()
                .any(|p| p.status == ModelDownloadStatus::Verifying),
            "must emit verifying progress after download"
        );
    }

    #[test]
    fn corrupted_model_triggers_redownload_and_verification() {
        let model_path = PathBuf::from("/tmp/models/model.bin");
        let store = MockStore::new(
            model_path.clone(),
            vec![
                Err(TranscribeError::ModelCorrupt {
                    detail: "checksum mismatch".to_string(),
                }),
                Ok(model_path.clone()),
            ],
        );
        let downloader = MockDownloader::success();

        let path = orchestrator(Arc::clone(&store), Arc::clone(&downloader))
            .ensure_model(|_| {})
            .expect("redownload should succeed");

        assert_eq!(path, model_path);
        assert_eq!(store.verify_call_count(), 2);
        assert_eq!(downloader.download_call_count(), 1);
    }

    #[test]
    fn download_failure_returns_model_download_failed() {
        let model_path = PathBuf::from("/tmp/models/model.bin");
        let store = MockStore::new(
            model_path,
            vec![Err(TranscribeError::ModelNotFound {
                detail: "missing".to_string(),
            })],
        );
        let downloader = MockDownloader::failure(TranscribeError::ModelDownloadFailed {
            detail: "network timeout".to_string(),
        });

        let err = orchestrator(Arc::clone(&store), Arc::clone(&downloader))
            .ensure_model(|_| {})
            .expect_err("download failure should propagate");

        assert!(matches!(err, TranscribeError::ModelDownloadFailed { .. }));
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::ModelDownloadFailed
        );
        assert_eq!(downloader.download_call_count(), 1);
    }

    #[test]
    fn post_download_checksum_failure_returns_model_corrupt() {
        let model_path = PathBuf::from("/tmp/models/model.bin");
        let store = MockStore::new(
            model_path,
            vec![
                Err(TranscribeError::ModelNotFound {
                    detail: "missing".to_string(),
                }),
                Err(TranscribeError::ModelCorrupt {
                    detail: "checksum mismatch after download".to_string(),
                }),
            ],
        );
        let downloader = MockDownloader::success();

        let err = orchestrator(Arc::clone(&store), Arc::clone(&downloader))
            .ensure_model(|_| {})
            .expect_err("post-download checksum failure should propagate");

        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        assert_eq!(err.to_user_facing().code, TranscribeErrorCode::ModelCorrupt);
        assert_eq!(downloader.download_call_count(), 1);
        assert_eq!(store.verify_call_count(), 2);
    }

    #[test]
    fn offline_first_launch_without_url_returns_model_not_found() {
        let model_path = PathBuf::from("/tmp/models/model.bin");
        let store = MockStore::new(
            model_path,
            vec![Err(TranscribeError::ModelNotFound {
                detail: "missing".to_string(),
            })],
        );
        let downloader = MockDownloader::success();
        let orchestrator = ModelOrchestrator::new(
            Arc::clone(&store),
            Arc::clone(&downloader),
            ModelOrchestratorConfig {
                model_url: String::new(),
                expected_sha256: EXPECTED_SHA.to_string(),
            },
        );

        let err = orchestrator
            .ensure_model(|_| {})
            .expect_err("offline first launch should fail");

        assert!(matches!(err, TranscribeError::ModelNotFound { .. }));
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::ModelNotFound
        );
        assert_eq!(downloader.download_call_count(), 0);
    }
}
