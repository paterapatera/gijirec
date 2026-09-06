//! Persists [`EditorSettings`] to `editor-settings.json` under a configurable data directory.

use gijirec_domain::editor::{EditorError, EditorSettings};
use std::path::{Path, PathBuf};

const SETTINGS_FILENAME: &str = "editor-settings.json";

/// Partial update: `None` fields are left unchanged on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorSettingsPatch {
    pub save_directory: Option<Option<String>>,
    pub export_jsonl_enabled: Option<bool>,
}

/// Reads and writes editor settings JSON under an injected base directory.
pub struct SettingsService {
    data_dir: PathBuf,
}

impl SettingsService {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir.join(SETTINGS_FILENAME)
    }

    pub fn get(&self) -> Result<EditorSettings, EditorError> {
        let path = self.settings_path();
        if !path.is_file() {
            return Ok(EditorSettings::default());
        }

        let contents =
            std::fs::read_to_string(&path).map_err(|err| persist_error("read settings", err))?;
        serde_json::from_str(&contents).map_err(|err| persist_error("parse settings", err))
    }

    pub fn save(&self, settings: &EditorSettings) -> Result<(), EditorError> {
        let path = self.settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| persist_error("create settings directory", err))?;
        }

        let json = serde_json::to_string_pretty(settings)
            .map_err(|err| persist_error("serialize settings", err))?;
        std::fs::write(&path, json).map_err(|err| persist_error("write settings", err))
    }

    pub fn update(&self, patch: EditorSettingsPatch) -> Result<EditorSettings, EditorError> {
        let mut current = self.get()?;
        if let Some(save_directory) = patch.save_directory {
            current.save_directory = save_directory;
        }
        if let Some(export_jsonl_enabled) = patch.export_jsonl_enabled {
            current.export_jsonl_enabled = export_jsonl_enabled;
        }
        self.save(&current)?;
        Ok(current)
    }
}

fn persist_error(action: &str, err: impl std::fmt::Display) -> EditorError {
    EditorError::SettingsPersistFailed {
        detail: format!("{action}: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorSettingsPatch, SettingsService};
    use gijirec_domain::editor::{EditorError, EditorSettings, EditorUserErrorCode};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_data_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gijirec-settings-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp data dir");
        dir
    }

    #[test]
    fn get_returns_defaults_when_file_missing() {
        let service = SettingsService::new(temp_data_dir());
        let settings = service.get().expect("get defaults");
        assert_eq!(settings, EditorSettings::default());
    }

    #[test]
    fn roundtrip_save_and_get() {
        let service = SettingsService::new(temp_data_dir());
        let expected = EditorSettings {
            save_directory: Some(r"C:\Users\me\exports".to_string()),
            export_jsonl_enabled: true,
        };
        service.save(&expected).expect("save settings");
        let restored = service.get().expect("get settings");
        assert_eq!(restored, expected);
    }

    #[test]
    fn partial_update_keeps_unset_fields() {
        let service = SettingsService::new(temp_data_dir());
        service
            .save(&EditorSettings {
                save_directory: Some("/data/saves".to_string()),
                export_jsonl_enabled: false,
            })
            .expect("seed settings");

        let updated = service
            .update(EditorSettingsPatch {
                save_directory: None,
                export_jsonl_enabled: Some(true),
            })
            .expect("partial update");

        assert_eq!(
            updated,
            EditorSettings {
                save_directory: Some("/data/saves".to_string()),
                export_jsonl_enabled: true,
            }
        );
    }

    #[test]
    fn persist_failure_maps_to_settings_persist_failed() {
        let blocking = temp_data_dir().join("blocked");
        std::fs::write(&blocking, "not-a-directory").expect("create blocking file");
        let service = SettingsService::new(blocking);

        let err = service
            .save(&EditorSettings::default())
            .expect_err("save must fail");

        assert!(matches!(err, EditorError::SettingsPersistFailed { .. }));
        assert_eq!(
            err.to_user_facing().code,
            EditorUserErrorCode::SettingsPersistFailed
        );
    }
}
