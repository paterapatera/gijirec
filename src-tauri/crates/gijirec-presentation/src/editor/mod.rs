//! Presentation adapters for the transcript editor (Tauri commands, UI glue).

pub mod commands;
pub mod observability;

pub use commands::{
    get_editor_settings_impl, pick_save_directory_from_selection, save_error_result,
    save_transcript_session_impl, set_editor_settings_impl,
};
pub use observability::{
    EDITOR_LOG_TARGET, EditorObservability, EditorSaveLogFields, log_save_completed,
    log_save_started, log_settings_updated, save_log_fields, set_editor_observability,
};
