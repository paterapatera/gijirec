use std::sync::{Arc, Mutex};

use gijirec_presentation::application::transcribe::model_orchestrator::ModelOrchestrator;
use gijirec_presentation::infrastructure::transcribe::{ModelDownloader, ModelStore};
use gijirec_presentation::transcribe::{ModelDownloaderPortAdapter, ModelStorePortAdapter};

/// Shared model acquisition handle used without holding the transcribe orchestrator lock.
pub(crate) type SharedModelOrchestrator =
    Arc<Mutex<ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter>>>;

pub(crate) fn build_model_orchestrator(
    app_data_dir: std::path::PathBuf,
) -> ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter> {
    let store = ModelStore::new(app_data_dir);
    let store_adapter = ModelStorePortAdapter::new(store);
    let downloader = ModelDownloader::new().expect("HTTPS client initialization");
    let downloader_adapter = ModelDownloaderPortAdapter::new(downloader).expect("adapter");
    ModelOrchestrator::new(store_adapter, downloader_adapter)
}

pub(crate) fn wrap_model_orchestrator(
    orchestrator: ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter>,
) -> SharedModelOrchestrator {
    Arc::new(Mutex::new(orchestrator))
}

/// Placeholder model stack until Tauri setup calls [`inject_model_stack`].
pub(crate) fn deferred_model_orchestrator() -> SharedModelOrchestrator {
    wrap_model_orchestrator(build_model_orchestrator(std::path::PathBuf::new()))
}

/// Injects the model stack into a shared handle (Tauri setup path).
pub(crate) fn inject_model_stack_shared(
    model_orchestrator: &SharedModelOrchestrator,
    app_data_dir: std::path::PathBuf,
) {
    *model_orchestrator.lock().expect("lock model orchestrator") =
        build_model_orchestrator(app_data_dir);
}
