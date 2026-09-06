//! Host tracing backend for transcript editor observability.

use gijirec_presentation::editor::observability::{
    EDITOR_LOG_TARGET, EditorObservability, EditorSaveCompletion, EditorSaveLogFields,
};

/// Emits structured editor events via `tracing` without transcript or handwriting bodies.
pub struct TracingEditorObservability;

impl EditorObservability for TracingEditorObservability {
    fn log_save_started(&self, fields: &EditorSaveLogFields) {
        tracing::info!(
            target: EDITOR_LOG_TARGET,
            event = "editor_save_started",
            session_id = fields.session_id.as_str(),
            handwriting_markdown_len = fields.handwriting_markdown_len,
            ai_transcription_markdown_len = fields.ai_transcription_markdown_len,
            jsonl_record_count = fields.jsonl_record_count,
            "editor save started"
        );
    }

    fn log_save_completed(&self, fields: &EditorSaveLogFields, completion: &EditorSaveCompletion) {
        tracing::info!(
            target: EDITOR_LOG_TARGET,
            event = "editor_save_completed",
            session_id = fields.session_id.as_str(),
            success = completion.success,
            files_written_count = completion.files_written_count,
            error_code = completion.error_code.map(|code| code.as_str()),
            "editor save completed"
        );
    }

    fn log_settings_updated(&self) {
        tracing::info!(
            target: EDITOR_LOG_TARGET,
            event = "settings_updated",
            "editor settings updated"
        );
    }
}
