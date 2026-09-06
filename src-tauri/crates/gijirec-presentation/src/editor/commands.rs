//! Testable editor command logic (Tauri wrappers live in the host `commands` module).

use crate::application::editor::{EditorSettingsPatch, SaveService, SettingsService};
use crate::domain::editor::{
    EditorError, EditorSettings, EditorUserError, SaveTranscriptSessionRequest,
    SaveTranscriptSessionResult,
};
use crate::editor::observability::{
    EditorSaveCompletion, log_save_completed, log_save_started, log_settings_updated,
    save_log_fields,
};
use std::path::PathBuf;

/// Maps [`EditorError`] from save into a failure [`SaveTranscriptSessionResult`].
pub fn save_error_result(err: EditorError) -> SaveTranscriptSessionResult {
    SaveTranscriptSessionResult {
        success: false,
        output_directory: None,
        files_written: None,
        files_failed: None,
        error: Some(err.to_user_facing()),
    }
}

/// Saves a transcript session snapshot using settings-backed save directory.
pub fn save_transcript_session_impl(
    settings_service: &SettingsService,
    request: SaveTranscriptSessionRequest,
) -> SaveTranscriptSessionResult {
    let jsonl_count = request.ai_transcription_jsonl.as_ref().map_or(0, Vec::len);
    let log_fields = save_log_fields(
        &request.session_id,
        &request.handwriting_markdown,
        &request.ai_transcription_markdown,
        jsonl_count,
    );
    log_save_started(&log_fields);

    let settings = match settings_service.get() {
        Ok(settings) => settings,
        Err(err) => {
            let result = save_error_result(err);
            log_save_completed(
                &log_fields,
                &EditorSaveCompletion {
                    success: false,
                    files_written_count: 0,
                    error_code: result.error.as_ref().map(|e| e.code),
                },
            );
            return result;
        }
    };

    let save_dir = settings.save_directory.map(PathBuf::from);
    let save_service = SaveService::new(save_dir);

    let result = match save_service.save(&request) {
        Ok(result) => result,
        Err(err) => save_error_result(err),
    };

    log_save_completed(
        &log_fields,
        &EditorSaveCompletion {
            success: result.success,
            files_written_count: result.files_written.as_ref().map_or(0, Vec::len),
            error_code: result.error.as_ref().map(|e| e.code),
        },
    );
    result
}

/// Loads persisted editor settings.
pub fn get_editor_settings_impl(
    settings_service: &SettingsService,
) -> Result<EditorSettings, EditorUserError> {
    settings_service.get().map_err(|err| err.to_user_facing())
}

/// Applies a partial settings update and persists to disk.
pub fn set_editor_settings_impl(
    settings_service: &SettingsService,
    save_directory: Option<Option<String>>,
    export_jsonl_enabled: Option<bool>,
) -> Result<EditorSettings, EditorUserError> {
    let patch = EditorSettingsPatch {
        save_directory,
        export_jsonl_enabled,
    };
    let updated = settings_service
        .update(patch)
        .map_err(|err| err.to_user_facing())?;
    log_settings_updated();
    Ok(updated)
}

/// Maps a folder-picker outcome to the invoke response (`null` when cancelled).
pub fn pick_save_directory_from_selection(selected: Option<PathBuf>) -> Option<String> {
    selected.map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        get_editor_settings_impl, pick_save_directory_from_selection, save_transcript_session_impl,
        set_editor_settings_impl,
    };
    use crate::application::editor::SettingsService;
    use crate::domain::editor::{
        EditorUserErrorCode, SaveTranscriptSessionRequest, SaveTranscriptSessionResult,
    };
    use std::fs;
    use std::path::PathBuf;
    fn temp_data_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-editor-cmd-test-{}-{}",
            label,
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp data dir");
        dir
    }

    fn temp_save_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-editor-save-test-{}-{}",
            label,
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp save dir");
        dir
    }

    #[test]
    fn save_directory_not_set_maps_to_result_error() {
        let service = SettingsService::new(temp_data_dir("not-set"));
        let secret = "これは手動議事録の全文です。";

        let result = save_transcript_session_impl(
            &service,
            SaveTranscriptSessionRequest {
                session_id: "session-1".to_string(),
                handwriting_markdown: secret.to_string(),
                ai_transcription_markdown: "ai body".to_string(),
                ai_transcription_jsonl: None,
            },
        );

        assert!(!result.success);
        let error = result.error.expect("error field");
        assert_eq!(error.code, EditorUserErrorCode::SaveDirectoryNotSet);
        assert!(!error.message_ja.contains(secret));
    }

    #[test]
    fn settings_roundtrip_via_command_impls() {
        let data_dir = temp_data_dir("roundtrip");
        let service = SettingsService::new(data_dir.clone());
        let save_path = temp_save_dir("roundtrip");
        let save_path_str = save_path.to_string_lossy().into_owned();

        let updated =
            set_editor_settings_impl(&service, Some(Some(save_path_str.clone())), Some(true))
                .expect("set settings");

        assert_eq!(updated.save_directory, Some(save_path_str.clone()));
        assert!(updated.export_jsonl_enabled);

        let loaded = get_editor_settings_impl(&service).expect("get settings");
        assert_eq!(loaded, updated);

        let settings_file = data_dir.join("editor-settings.json");
        assert!(settings_file.is_file());
        let contents = fs::read_to_string(settings_file).expect("read settings json");
        assert!(contents.contains("export_jsonl_enabled"));
        let persisted: crate::domain::editor::EditorSettings =
            serde_json::from_str(&contents).expect("parse settings json");
        assert_eq!(persisted, updated);
    }

    #[test]
    fn pick_save_directory_cancel_returns_null() {
        assert_eq!(pick_save_directory_from_selection(None), None);
    }

    #[test]
    fn pick_save_directory_selection_returns_path_string() {
        let dir = temp_save_dir("pick");
        let selected = pick_save_directory_from_selection(Some(dir.clone()));
        assert_eq!(selected, Some(dir.to_string_lossy().into_owned()));
    }

    #[test]
    fn save_success_returns_contract_result_shape() {
        let data_dir = temp_data_dir("save-ok");
        let save_dir = temp_save_dir("save-ok");
        let service = SettingsService::new(data_dir);
        set_editor_settings_impl(
            &service,
            Some(Some(save_dir.to_string_lossy().into_owned())),
            None,
        )
        .expect("seed save directory");

        let result = save_transcript_session_impl(
            &service,
            SaveTranscriptSessionRequest {
                session_id: "session-ok".to_string(),
                handwriting_markdown: "# notes".to_string(),
                ai_transcription_markdown: "transcript".to_string(),
                ai_transcription_jsonl: None,
            },
        );

        assert!(result.success);
        assert!(result.error.is_none());
        assert!(result.output_directory.is_some());
        assert!(result.files_written.is_some());
    }

    #[test]
    fn save_error_result_has_success_false() {
        let result: SaveTranscriptSessionResult =
            super::save_error_result(crate::domain::editor::EditorError::SaveDirectoryNotSet);
        assert!(!result.success);
        assert_eq!(
            result.error.expect("error").code,
            EditorUserErrorCode::SaveDirectoryNotSet
        );
    }
}
