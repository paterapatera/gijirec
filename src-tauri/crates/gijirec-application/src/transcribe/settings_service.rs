//! Persists [`TranscribeSettings`] to `transcribe-settings.json` under `app_data_dir`.

use gijirec_domain::transcribe::{
    TranscribeSettings, TranscribeSettingsError, TranscribeSettingsLoadIssue,
    TranscribeSettingsLoadResult,
};

const SETTINGS_FILENAME: &str = "transcribe-settings.json";

crate::settings_service_shell!(pub TranscribeSettingsService, SETTINGS_FILENAME);

impl TranscribeSettingsService {
    /// Loads settings. Missing or corrupt files yield FP16 default with a recorded issue.
    pub fn load(&self) -> TranscribeSettingsLoadResult {
        let path = self.settings_path();
        if !path.is_file() {
            return TranscribeSettingsLoadResult {
                settings: TranscribeSettings::default(),
                issue: Some(TranscribeSettingsLoadIssue::FileMissing),
            };
        }

        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<TranscribeSettings>(&contents) {
                Ok(settings) => TranscribeSettingsLoadResult {
                    settings,
                    issue: None,
                },
                Err(err) => TranscribeSettingsLoadResult {
                    settings: TranscribeSettings::default(),
                    issue: Some(TranscribeSettingsLoadIssue::ParseError {
                        detail: err.to_string(),
                    }),
                },
            },
            Err(err) => TranscribeSettingsLoadResult {
                settings: TranscribeSettings::default(),
                issue: Some(TranscribeSettingsLoadIssue::ParseError {
                    detail: err.to_string(),
                }),
            },
        }
    }

    pub fn save(&self, settings: &TranscribeSettings) -> Result<(), TranscribeSettingsError> {
        crate::settings_file::write_json_pretty(&self.settings_path(), settings, |detail| {
            TranscribeSettingsError::SettingsPersistFailed { detail }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::TranscribeSettingsService;
    use gijirec_domain::transcribe::{
        TranscribeSettings, TranscribeSettingsError, TranscribeSettingsErrorCode,
        TranscribeSettingsLoadIssue, WhisperModelVariant,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_data_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gijirec-transcribe-settings-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp data dir");
        dir
    }

    #[test]
    fn load_returns_fp16_default_when_file_missing() {
        let service = TranscribeSettingsService::new(temp_data_dir());
        let result = service.load();
        assert_eq!(result.settings, TranscribeSettings::default());
        assert!(matches!(
            result.issue,
            Some(TranscribeSettingsLoadIssue::FileMissing)
        ));
    }

    #[test]
    fn load_returns_fp16_default_and_issue_on_corrupt_json() {
        let dir = temp_data_dir();
        let path = dir.join("transcribe-settings.json");
        std::fs::write(&path, "{ not valid json").expect("write corrupt file");

        let service = TranscribeSettingsService::new(dir);
        let result = service.load();
        assert_eq!(result.settings.model_variant, WhisperModelVariant::Fp16);
        assert!(matches!(
            result.issue,
            Some(TranscribeSettingsLoadIssue::ParseError { .. })
        ));
    }

    #[test]
    fn roundtrip_save_and_load() {
        let service = TranscribeSettingsService::new(temp_data_dir());
        let expected = TranscribeSettings {
            model_variant: WhisperModelVariant::Q8_0,
        };
        service.save(&expected).expect("save settings");

        let result = service.load();
        assert_eq!(result.settings, expected);
        assert!(result.issue.is_none());
    }

    #[test]
    fn save_failure_maps_to_settings_persist_failed() {
        let blocking = temp_data_dir().join("blocked");
        std::fs::write(&blocking, "not-a-directory").expect("create blocking file");
        let service = TranscribeSettingsService::new(blocking);

        let err = service
            .save(&TranscribeSettings::default())
            .expect_err("save must fail");

        assert!(matches!(
            err,
            TranscribeSettingsError::SettingsPersistFailed { .. }
        ));
        assert_eq!(
            err.to_user_facing().code,
            TranscribeSettingsErrorCode::SettingsPersistFailed
        );
    }

    #[test]
    fn saved_json_contains_only_model_variant() {
        let dir = temp_data_dir();
        let service = TranscribeSettingsService::new(dir.clone());
        service
            .save(&TranscribeSettings {
                model_variant: WhisperModelVariant::Q5_0,
            })
            .expect("save");

        let contents =
            std::fs::read_to_string(dir.join("transcribe-settings.json")).expect("read file");
        let value: serde_json::Value = serde_json::from_str(&contents).expect("parse json");
        let obj = value.as_object().expect("object");
        assert_eq!(obj.len(), 1);
        assert_eq!(
            obj.get("model_variant").and_then(|v| v.as_str()),
            Some("q5_0")
        );
    }
}
