//! Integration tests for whisper-model-selection (design Testing Strategy — Integration).

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use gijirec_presentation::application::transcribe::model_orchestrator::{
    ApplyVariantOutcome, ModelOrchestrator, ModelOrchestratorConfig,
};
use gijirec_presentation::application::transcribe::ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
};
use gijirec_presentation::application::transcribe::TranscribeSettingsService;
use gijirec_presentation::domain::transcribe::{
    TranscribeError, TranscribeSettings, WhisperModelVariant,
};
use gijirec_presentation::transcribe::persist_transcribe_model_variant;

struct SequenceStore {
    path: PathBuf,
    verify_calls: AtomicUsize,
    fail_first: bool,
}

impl SequenceStore {
    fn missing_then_ok(path: PathBuf) -> Self {
        Self {
            path,
            verify_calls: AtomicUsize::new(0),
            fail_first: true,
        }
    }
}

impl ModelStorePort for SequenceStore {
    fn model_path(&self) -> PathBuf {
        self.path.clone()
    }

    fn model_path_for(&self, _variant: WhisperModelVariant) -> PathBuf {
        self.path.clone()
    }

    fn verify(&self, expected: Option<&str>) -> Result<PathBuf, TranscribeError> {
        self.verify_variant(WhisperModelVariant::Fp16, expected)
    }

    fn verify_variant(
        &self,
        _variant: WhisperModelVariant,
        _expected: Option<&str>,
    ) -> Result<PathBuf, TranscribeError> {
        self.verify_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_first && self.verify_calls.load(Ordering::SeqCst) == 1 {
            return Err(TranscribeError::ModelNotFound {
                detail: "missing".to_string(),
            });
        }
        Ok(self.path.clone())
    }

    fn file_exists(&self, _variant: WhisperModelVariant) -> bool {
        false
    }
}

struct OkDownloader {
    calls: AtomicUsize,
}

impl ModelDownloaderPort for OkDownloader {
    fn download(
        &self,
        _url: &str,
        _destination: &std::path::Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        on_progress(ModelDownloadProgress {
            bytes_downloaded: 100,
            bytes_total: Some(100),
            percent: Some(100.0),
            status: ModelDownloadStatus::Downloading,
        });
        Ok(())
    }
}

fn temp_data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gijirec-wms-int-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn startup_persisted_settings_initialize_orchestrator_variant() {
    let data_dir = temp_data_dir("startup");
    let service = TranscribeSettingsService::new(data_dir);
    service
        .save(&TranscribeSettings {
            model_variant: WhisperModelVariant::Q8_0,
        })
        .expect("save settings");

    let load = service.load();
    assert_eq!(load.settings.model_variant, WhisperModelVariant::Q8_0);

    let path = PathBuf::from("/tmp/models/q8.bin");
    let store = SequenceStore {
        path: path.clone(),
        verify_calls: AtomicUsize::new(0),
        fail_first: false,
    };
    let mut orchestrator = ModelOrchestrator::new(
        store,
        OkDownloader {
            calls: AtomicUsize::new(0),
        },
        ModelOrchestratorConfig::fp16_from_catalog(),
    );
    orchestrator.initialize_selected_variant(load.settings.model_variant);
    assert_eq!(orchestrator.selected_variant(), WhisperModelVariant::Q8_0);
}

#[test]
fn set_variant_persists_and_downloads_when_local_model_missing() {
    let data_dir = temp_data_dir("set-variant");
    let service = TranscribeSettingsService::new(data_dir);
    let path = PathBuf::from("/tmp/models/variant.bin");
    let downloader = OkDownloader {
        calls: AtomicUsize::new(0),
    };
    let mut orchestrator = ModelOrchestrator::new(
        SequenceStore::missing_then_ok(path.clone()),
        downloader,
        ModelOrchestratorConfig::fp16_from_catalog(),
    );

    let settings = persist_transcribe_model_variant(&service, WhisperModelVariant::Q5_0)
        .expect("persist");
    assert_eq!(settings.model_variant, WhisperModelVariant::Q5_0);

    let progress = Mutex::new(Vec::<ModelDownloadProgress>::new());
    orchestrator
        .apply_variant(WhisperModelVariant::Q5_0, false, |update| {
            progress.lock().expect("lock").push(update);
        })
        .expect("apply variant");

    assert!(
        progress
            .lock()
            .expect("lock")
            .iter()
            .any(|p| p.status == ModelDownloadStatus::Downloading),
        "download progress must be emitted"
    );
    assert_eq!(
        service.load().settings.model_variant,
        WhisperModelVariant::Q5_0
    );

    let outcome = orchestrator
        .apply_variant(WhisperModelVariant::Q5_0, false, |_| {})
        .expect("re-apply same variant");
    assert!(matches!(outcome, ApplyVariantOutcome::NoOp { .. }));
}
