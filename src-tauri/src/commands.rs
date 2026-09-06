//! Tauri IPC commands for capture and transcribe status.

use gijirec_presentation::application::editor::SettingsService;
use gijirec_presentation::domain::editor::{
    AiTranscriptionJsonlRecord, EditorSettings, EditorUserError, SaveTranscriptSessionRequest,
    SaveTranscriptSessionResult,
};
use gijirec_presentation::editor::{
    get_editor_settings_impl, pick_save_directory_from_selection, save_transcript_session_impl,
    set_editor_settings_impl,
};
use gijirec_presentation::tauri::events::CapturePhaseChangedPayload;
use gijirec_presentation::tauri::lifecycle::CaptureLifecycleState;
use gijirec_presentation::transcribe::event_emitter::TranscribePhaseChangedPayload;
use gijirec_presentation::transcribe::{TranscribeStatusCache, TranscribeStatusSnapshot};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

fn dialog_folder_to_path(file_path: FilePath) -> Option<PathBuf> {
    file_path.into_path().ok()
}

/// Shared editor settings service (backed by `app_data_dir`).
pub struct EditorState {
    pub settings_service: Arc<SettingsService>,
}

/// Editor IPC commands (`docs/contracts/transcript-editor-save.md`, `transcript-editor-settings.md`).
pub mod editor {
    use super::*;

    /// Persists handwriting / AI transcript files under the configured save directory.
    ///
    /// `rename_all = "snake_case"` matches `docs/contracts/transcript-editor-save.md`
    /// (Tauri 2 defaults to camelCase IPC keys).
    #[tauri::command(rename_all = "snake_case")]
    #[allow(clippy::too_many_arguments)] // IPC contract mirrors `SaveTranscriptSessionRequest` fields.
    pub fn save_transcript_session(
        state: State<'_, EditorState>,
        session_id: String,
        handwriting_markdown: String,
        ai_transcription_markdown: String,
        ai_transcription_jsonl: Option<Vec<AiTranscriptionJsonlRecord>>,
    ) -> SaveTranscriptSessionResult {
        save_transcript_session_impl(
            &state.settings_service,
            SaveTranscriptSessionRequest {
                session_id,
                handwriting_markdown,
                ai_transcription_markdown,
                ai_transcription_jsonl,
            },
        )
    }

    /// Returns persisted editor settings from `app_data_dir/editor-settings.json`.
    #[tauri::command]
    pub fn get_editor_settings(
        state: State<'_, EditorState>,
    ) -> Result<EditorSettings, EditorUserError> {
        get_editor_settings_impl(&state.settings_service)
    }

    /// Partially updates editor settings and persists to disk.
    ///
    /// `rename_all = "snake_case"` matches `docs/contracts/transcript-editor-settings.md`.
    #[tauri::command(rename_all = "snake_case")]
    pub fn set_editor_settings(
        state: State<'_, EditorState>,
        save_directory: Option<String>,
        export_jsonl_enabled: Option<bool>,
    ) -> Result<EditorSettings, EditorUserError> {
        set_editor_settings_impl(
            &state.settings_service,
            save_directory.map(Some),
            export_jsonl_enabled,
        )
    }

    /// Opens a native folder picker; returns `null` when cancelled (does not auto-persist).
    ///
    /// Must be `async` so `blocking_pick_folder` runs off the UI thread (plugin contract).
    #[tauri::command]
    pub async fn pick_save_directory(app: AppHandle) -> Option<String> {
        let mut dialog = app.dialog().file();
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
        let selected = dialog
            .blocking_pick_folder()
            .and_then(dialog_folder_to_path);
        pick_save_directory_from_selection(selected)
    }
}

/// Returns the orchestrator's current phase (sync on frontend mount).
#[tauri::command]
pub fn get_capture_phase(
    state: State<'_, Arc<CaptureLifecycleState>>,
) -> Result<CapturePhaseChangedPayload, String> {
    state.current_phase_payload()
}

/// Returns the cached transcribe phase (non-blocking during model download).
#[tauri::command]
pub fn get_transcribe_phase(
    cache: State<'_, Arc<TranscribeStatusCache>>,
) -> Result<TranscribePhaseChangedPayload, String> {
    Ok(cache.phase_payload())
}

/// Returns cached transcribe status for frontend mount sync.
#[tauri::command]
pub fn get_transcribe_status(
    cache: State<'_, Arc<TranscribeStatusCache>>,
) -> Result<TranscribeStatusSnapshot, String> {
    Ok(cache.snapshot())
}

/// Device selection IPC commands (`docs/contracts/audio-device-selection.md`).
pub mod device_selection {
    use super::*;
    use gijirec_presentation::application::device_selection::DeviceSelectionService;
    use gijirec_presentation::domain::audio::{AudioDeviceId, AudioDeviceList, DeviceSelection};
    use gijirec_presentation::tauri::device_selection::{
        DeviceSelectionInvokeError, get_device_selection_impl, list_audio_devices_impl,
        set_audio_device_ui_visible_impl, set_device_selection_impl,
    };

    #[tauri::command]
    pub fn list_audio_devices(
        service: State<'_, Arc<dyn DeviceSelectionService>>,
    ) -> Result<AudioDeviceList, DeviceSelectionInvokeError> {
        list_audio_devices_impl(service.inner().as_ref())
    }

    #[tauri::command]
    pub fn get_device_selection(
        service: State<'_, Arc<dyn DeviceSelectionService>>,
    ) -> DeviceSelection {
        get_device_selection_impl(service.inner().as_ref())
    }

    #[tauri::command(rename_all = "snake_case")]
    pub fn set_device_selection(
        service: State<'_, Arc<dyn DeviceSelectionService>>,
        microphone_id: Option<AudioDeviceId>,
        speaker_id: Option<AudioDeviceId>,
    ) -> Result<DeviceSelection, DeviceSelectionInvokeError> {
        set_device_selection_impl(
            service.inner().as_ref(),
            DeviceSelection::new(microphone_id, speaker_id),
        )
    }

    #[tauri::command(rename_all = "snake_case")]
    pub fn set_audio_device_ui_visible(
        service: State<'_, Arc<dyn DeviceSelectionService>>,
        visible: bool,
    ) {
        set_audio_device_ui_visible_impl(service.inner().as_ref(), visible);
    }
}
