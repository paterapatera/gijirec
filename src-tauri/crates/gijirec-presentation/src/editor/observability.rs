//! Editor observability dispatch (bylaw-safe in presentation — no tracing macros).

use gijirec_domain::editor::EditorUserErrorCode;
use std::sync::{OnceLock, RwLock};

/// Log-safe fields for save lifecycle events (no markdown bodies).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EditorSaveLogFields {
    pub session_id: String,
    pub handwriting_markdown_len: usize,
    pub ai_transcription_markdown_len: usize,
    pub jsonl_record_count: usize,
}

/// Builds masked log fields for `editor_save_started` / `editor_save_completed`.
pub fn save_log_fields(
    session_id: &str,
    handwriting_markdown: &str,
    ai_transcription_markdown: &str,
    jsonl_record_count: usize,
) -> EditorSaveLogFields {
    EditorSaveLogFields {
        session_id: session_id.to_string(),
        handwriting_markdown_len: handwriting_markdown.len(),
        ai_transcription_markdown_len: ai_transcription_markdown.len(),
        jsonl_record_count,
    }
}

/// Target name for editor host tracing (`RUST_LOG=gijirec_editor=info`).
pub const EDITOR_LOG_TARGET: &str = "gijirec_editor";

/// Completion metadata for `editor_save_completed` (no document bodies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSaveCompletion {
    pub success: bool,
    pub files_written_count: usize,
    pub error_code: Option<EditorUserErrorCode>,
}

/// Structured editor observability hooks.
pub trait EditorObservability: Send + Sync {
    fn log_save_started(&self, fields: &EditorSaveLogFields);
    fn log_save_completed(&self, fields: &EditorSaveLogFields, completion: &EditorSaveCompletion);
    fn log_settings_updated(&self);
}

struct NoopEditorObservability;

impl EditorObservability for NoopEditorObservability {
    fn log_save_started(&self, _fields: &EditorSaveLogFields) {}
    fn log_save_completed(
        &self,
        _fields: &EditorSaveLogFields,
        _completion: &EditorSaveCompletion,
    ) {
    }
    fn log_settings_updated(&self) {}
}

static EDITOR_OBSERVABILITY: OnceLock<RwLock<Box<dyn EditorObservability>>> = OnceLock::new();

fn editor_observability() -> &'static RwLock<Box<dyn EditorObservability>> {
    EDITOR_OBSERVABILITY.get_or_init(|| RwLock::new(Box::new(NoopEditorObservability)))
}

/// Registers or replaces the editor observability backend.
pub fn set_editor_observability(backend: Box<dyn EditorObservability>) {
    if let Some(lock) = EDITOR_OBSERVABILITY.get() {
        *lock.write().expect("lock") = backend;
    } else {
        let _ = EDITOR_OBSERVABILITY.set(RwLock::new(backend));
    }
}

/// Logs the start of a transcript save (no document bodies).
pub fn log_save_started(fields: &EditorSaveLogFields) {
    editor_observability()
        .read()
        .expect("lock")
        .log_save_started(fields);
}

/// Logs save completion with file count and optional error code (no document bodies).
pub fn log_save_completed(fields: &EditorSaveLogFields, completion: &EditorSaveCompletion) {
    editor_observability()
        .read()
        .expect("lock")
        .log_save_completed(fields, completion);
}

/// Logs that editor settings were persisted.
pub fn log_settings_updated() {
    editor_observability()
        .read()
        .expect("lock")
        .log_settings_updated();
}

#[cfg(test)]
mod tests {
    use super::{EditorSaveLogFields, save_log_fields};

    #[test]
    fn save_log_fields_omit_markdown_bodies() {
        let secret_handwriting = "これは手動議事録の全文です。";
        let secret_ai = "これはAI転写の全文です。";
        let fields = save_log_fields("session-abc", secret_handwriting, secret_ai, 3);

        assert_eq!(
            fields,
            EditorSaveLogFields {
                session_id: "session-abc".to_string(),
                handwriting_markdown_len: secret_handwriting.len(),
                ai_transcription_markdown_len: secret_ai.len(),
                jsonl_record_count: 3,
            }
        );

        let serialized = format!("{fields:?}");
        assert!(!serialized.contains(secret_handwriting));
        assert!(!serialized.contains(secret_ai));
    }
}
