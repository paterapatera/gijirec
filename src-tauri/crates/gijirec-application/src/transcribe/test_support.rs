//! Shared transcribe test fixtures for application-layer unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gijirec_domain::transcribe::{ModelVariantCatalog, TranscribeError, WhisperModelVariant};

use super::ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
};

pub(crate) const DEFAULT_MODEL_PATH: &str = "/tmp/models/model.bin";

pub(crate) struct NoopModelDownloader;

/* jscpd:ignore-start */
impl ModelDownloaderPort for NoopModelDownloader {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        noop_model_download(url, destination, on_progress)
    }
}
/* jscpd:ignore-end */

pub(crate) fn default_model_path() -> PathBuf {
    PathBuf::from(DEFAULT_MODEL_PATH)
}

pub(crate) struct QueueModelStore {
    model_path: PathBuf,
    verify_results: Mutex<Vec<Result<PathBuf, TranscribeError>>>,
    verify_calls: AtomicUsize,
    assert_fp16_sha: bool,
}

impl QueueModelStore {
    fn build(
        model_path: PathBuf,
        verify_results: Vec<Result<PathBuf, TranscribeError>>,
        assert_fp16_sha: bool,
    ) -> Self {
        Self {
            model_path,
            verify_results: Mutex::new(verify_results),
            verify_calls: AtomicUsize::new(0),
            assert_fp16_sha,
        }
    }

    pub(crate) fn new(
        model_path: PathBuf,
        verify_results: Vec<Result<PathBuf, TranscribeError>>,
    ) -> Arc<Self> {
        Arc::new(Self::build(model_path, verify_results, true))
    }

    pub(crate) fn with_valid_model() -> Arc<Self> {
        let model_path = default_model_path();
        Self::new(model_path.clone(), vec![Ok(model_path)])
    }

    pub(crate) fn without_verify_tracking(
        model_path: PathBuf,
        verify_results: Vec<Result<PathBuf, TranscribeError>>,
    ) -> Arc<Self> {
        Arc::new(Self::build(model_path, verify_results, false))
    }

    pub(crate) fn verify_call_count(self: &Arc<Self>) -> usize {
        self.verify_calls.load(Ordering::SeqCst)
    }
}

impl ModelStorePort for Arc<QueueModelStore> {
    fn model_path(&self) -> PathBuf {
        self.model_path.clone()
    }

    fn model_path_for(&self, _variant: WhisperModelVariant) -> PathBuf {
        self.model_path.clone()
    }

    fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
        self.verify_variant(WhisperModelVariant::Fp16, expected_sha256)
    }

    fn verify_variant(
        &self,
        _variant: WhisperModelVariant,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf, TranscribeError> {
        self.verify_calls.fetch_add(1, Ordering::SeqCst);
        if self.assert_fp16_sha {
            assert_eq!(
                expected_sha256,
                Some(ModelVariantCatalog::fp16().expected_sha256)
            );
        }

        let mut results = self.verify_results.lock().expect("lock verify results");
        if results.is_empty() {
            panic!("unexpected verify call");
        }
        results.remove(0)
    }

    fn file_exists(&self, _variant: WhisperModelVariant) -> bool {
        false
    }
}

pub(crate) struct QueueModelDownloader {
    download_calls: AtomicUsize,
    result: Mutex<Result<(), TranscribeError>>,
    emit_progress: bool,
}

impl QueueModelDownloader {
    pub(crate) fn success() -> Arc<Self> {
        Arc::new(Self {
            download_calls: AtomicUsize::new(0),
            result: Mutex::new(Ok(())),
            emit_progress: true,
        })
    }

    pub(crate) fn failure(err: TranscribeError) -> Arc<Self> {
        Arc::new(Self {
            download_calls: AtomicUsize::new(0),
            result: Mutex::new(Err(err)),
            emit_progress: true,
        })
    }

    pub(crate) fn download_call_count(self: &Arc<Self>) -> usize {
        self.download_calls.load(Ordering::SeqCst)
    }
}

impl ModelDownloaderPort for Arc<QueueModelDownloader> {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        self.download_calls.fetch_add(1, Ordering::SeqCst);
        if self.emit_progress {
            assert_eq!(url, ModelVariantCatalog::fp16().url);
            assert_eq!(destination, Path::new(DEFAULT_MODEL_PATH));
            on_progress(ModelDownloadProgress {
                bytes_downloaded: 100,
                bytes_total: Some(100),
                percent: Some(100.0),
                status: ModelDownloadStatus::Downloading,
            });
        }
        let result = self.result.lock().expect("lock download result").clone();
        if self.emit_progress && result.is_ok() {
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

fn noop_model_download(
    _url: &str,
    _destination: &Path,
    _on_progress: &mut dyn FnMut(ModelDownloadProgress),
) -> Result<(), TranscribeError> {
    Ok(())
}
