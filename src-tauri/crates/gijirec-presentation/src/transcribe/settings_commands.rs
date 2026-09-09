//! Testable transcribe settings command logic (Tauri wrappers live in the host `commands` module).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use gijirec_application::transcribe::{
    ApplyVariantOutcome, ModelDownloadProgress, ModelOrchestrator, ModelStorePort,
    TranscribeOrchestrator, TranscribeSettingsService,
};
use gijirec_domain::transcribe::{
    TranscribeError, TranscribePhase, TranscribeSettings, TranscribeSettingsError,
    TranscribeSettingsUserError, WhisperModelVariant,
};

use super::observability::{log_model_variant_applied, log_model_variant_selected};

/// `get_transcribe_settings` response per `docs/contracts/whisper-transcribe-settings.md`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GetTranscribeSettingsResponse {
    pub settings: TranscribeSettings,
    pub local_availability: HashMap<WhisperModelVariant, bool>,
}

/// `set_transcribe_model_variant` response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SetTranscribeModelVariantResponse {
    pub settings: TranscribeSettings,
}

/// Loads persisted settings and local file availability for all catalog variants.
pub fn get_transcribe_settings_impl(
    settings_service: &TranscribeSettingsService,
    local_availability: std::collections::HashMap<WhisperModelVariant, bool>,
) -> GetTranscribeSettingsResponse {
    let load_result = settings_service.load();
    GetTranscribeSettingsResponse {
        settings: load_result.settings,
        local_availability,
    }
}

/// Persists the selected model variant to `transcribe-settings.json`.
pub fn persist_transcribe_model_variant(
    settings_service: &TranscribeSettingsService,
    model_variant: WhisperModelVariant,
) -> Result<TranscribeSettings, TranscribeSettingsUserError> {
    let settings = TranscribeSettings { model_variant };
    settings_service
        .save(&settings)
        .map_err(|err| err.to_user_facing())?;
    Ok(settings)
}

/// Applies a persisted variant via [`ModelOrchestrator`] and prepares the worker when not deferred.
pub fn apply_transcribe_model_variant_impl<S, D>(
    model_orchestrator: &Arc<Mutex<ModelOrchestrator<S, D>>>,
    transcribe_orchestrator: &Arc<Mutex<dyn TranscribeOrchestrator>>,
    model_variant: WhisperModelVariant,
    on_progress: impl FnMut(ModelDownloadProgress),
) -> Result<(), TranscribeError>
where
    S: ModelStorePort,
    D: gijirec_application::transcribe::ModelDownloaderPort,
{
    let defer_to_cycle_boundary = {
        let orch = transcribe_orchestrator
            .lock()
            .expect("lock transcribe orchestrator");
        orch.phase() == TranscribePhase::Transcribing
    };

    log_model_variant_selected(model_variant);

    let outcome = {
        let mut model = model_orchestrator.lock().expect("lock model orchestrator");
        model.apply_variant(model_variant, defer_to_cycle_boundary, on_progress)?
    };

    match outcome {
        ApplyVariantOutcome::NoOp { path } => {
            if !defer_to_cycle_boundary {
                prepare_worker_path(transcribe_orchestrator, &path)?;
            }
        }
        ApplyVariantOutcome::Deferred { .. } => {}
        ApplyVariantOutcome::Applied { path } => {
            log_model_variant_applied(model_variant);
            prepare_worker_path(transcribe_orchestrator, &path)?;
        }
    }

    Ok(())
}

fn prepare_worker_path(
    transcribe_orchestrator: &Arc<Mutex<dyn TranscribeOrchestrator>>,
    path: &Path,
) -> Result<(), TranscribeError> {
    let mut orch = transcribe_orchestrator
        .lock()
        .expect("lock transcribe orchestrator");
    orch.begin_model_loading()?;
    orch.finish_model_loading(path)
}

/// Maps serde failures for invalid variant strings into contract error codes.
pub fn invalid_model_variant_error(detail: String) -> TranscribeSettingsUserError {
    TranscribeSettingsError::InvalidModelVariant { detail }.to_user_facing()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn temp_data_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-transcribe-settings-cmd-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn get_returns_settings_and_local_availability_for_three_variants() {
        let data_dir = temp_data_dir();
        let service = TranscribeSettingsService::new(data_dir.clone());
        let availability = HashMap::from([
            (WhisperModelVariant::Fp16, true),
            (WhisperModelVariant::Q8_0, false),
            (WhisperModelVariant::Q5_0, false),
        ]);

        let response = get_transcribe_settings_impl(&service, availability);
        assert_eq!(response.settings.model_variant, WhisperModelVariant::Fp16);
        assert_eq!(response.local_availability.len(), 3);
        assert_eq!(
            response.local_availability.get(&WhisperModelVariant::Fp16),
            Some(&true)
        );
        assert_eq!(
            response.local_availability.get(&WhisperModelVariant::Q8_0),
            Some(&false)
        );
    }

    #[test]
    fn persist_maps_invalid_variant_to_contract_code() {
        let err = invalid_model_variant_error("q4_0".to_string());
        assert_eq!(
            err.code,
            gijirec_domain::transcribe::TranscribeSettingsErrorCode::InvalidModelVariant
        );
    }
}
