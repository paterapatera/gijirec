//! Save session request/result types (`docs/contracts/transcript-editor-save.md`).

use crate::editor::error::EditorUserError;

/// One JSONL line in `ai-transcription.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AiTranscriptionJsonlRecord {
    pub block_id: String,
    pub sequence: u64,
    pub text: String,
    pub start_timestamp_ms: u64,
    pub language: String,
}

/// Snapshot passed to `save_transcript_session`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaveTranscriptSessionRequest {
    pub session_id: String,
    pub handwriting_markdown: String,
    pub ai_transcription_markdown: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_transcription_jsonl: Option<Vec<AiTranscriptionJsonlRecord>>,
}

/// Per-file failure entry when save partially succeeds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaveFileFailure {
    pub path: String,
    pub reason_ja: String,
}

/// Outcome of `save_transcript_session`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SaveTranscriptSessionResult {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_written: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_failed: Option<Vec<SaveFileFailure>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<EditorUserError>,
}

#[cfg(test)]
mod tests {
    use super::{
        AiTranscriptionJsonlRecord, SaveFileFailure, SaveTranscriptSessionRequest,
        SaveTranscriptSessionResult,
    };
    use crate::editor::error::{EditorUserError, EditorUserErrorCode};
    use serde_json::{Value, json};

    fn sample_jsonl_record() -> AiTranscriptionJsonlRecord {
        AiTranscriptionJsonlRecord {
            block_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            sequence: 1,
            text: "hello".to_string(),
            start_timestamp_ms: 1_500,
            language: "ja".to_string(),
        }
    }

    #[test]
    fn jsonl_record_serializes_contract_field_names() {
        let record = sample_jsonl_record();
        let value: Value = serde_json::to_value(&record).expect("serialize jsonl record");
        let obj = value.as_object().expect("object");
        assert!(obj.contains_key("block_id"));
        assert!(obj.contains_key("sequence"));
        assert!(obj.contains_key("text"));
        assert!(obj.contains_key("start_timestamp_ms"));
        assert!(obj.contains_key("language"));
        assert_eq!(obj.len(), 5);
    }

    #[test]
    fn save_request_round_trips_with_optional_jsonl() {
        let original = SaveTranscriptSessionRequest {
            session_id: "session-abc".to_string(),
            handwriting_markdown: "# notes".to_string(),
            ai_transcription_markdown: "transcript".to_string(),
            ai_transcription_jsonl: Some(vec![sample_jsonl_record()]),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: SaveTranscriptSessionRequest =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn save_request_omits_jsonl_when_none() {
        let request = SaveTranscriptSessionRequest {
            session_id: "session-abc".to_string(),
            handwriting_markdown: String::new(),
            ai_transcription_markdown: String::new(),
            ai_transcription_jsonl: None,
        };
        let value: Value = serde_json::to_value(&request).expect("serialize");
        let obj = value.as_object().expect("object");
        assert!(!obj.contains_key("ai_transcription_jsonl"));
    }

    #[test]
    fn save_result_success_shape_round_trips() {
        let original = SaveTranscriptSessionResult {
            success: true,
            output_directory: Some(r"C:\out\2026\09\06\10_45_00".to_string()),
            files_written: Some(vec![
                "handwriting.md".to_string(),
                "ai-transcription.md".to_string(),
            ]),
            files_failed: None,
            error: None,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: SaveTranscriptSessionResult =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn save_result_failure_includes_editor_user_error_stub() {
        let original = SaveTranscriptSessionResult {
            success: false,
            output_directory: None,
            files_written: Some(vec!["handwriting.md".to_string()]),
            files_failed: Some(vec![SaveFileFailure {
                path: "ai-transcription.md".to_string(),
                reason_ja: "書き込みに失敗しました。".to_string(),
            }]),
            error: Some(EditorUserError {
                code: EditorUserErrorCode::SavePartialFailure,
                message_ja: "一部のファイルの保存に失敗しました。".to_string(),
                action_ja: "保存先を確認してください。".to_string(),
                recoverable: true,
            }),
        };
        let value: Value = serde_json::to_value(&original).expect("serialize");
        let error = value
            .get("error")
            .and_then(Value::as_object)
            .expect("error object");
        assert_eq!(
            error.get("code").and_then(Value::as_str),
            Some("SAVE_PARTIAL_FAILURE")
        );
        assert!(error.get("message_ja").and_then(Value::as_str).is_some());
        assert!(error.get("action_ja").and_then(Value::as_str).is_some());
        assert_eq!(
            error.get("recoverable").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn editor_user_error_deserializes_from_contract_json() {
        let restored: EditorUserError = serde_json::from_value(json!({
            "code": "SAVE_DIRECTORY_NOT_SET",
            "message_ja": "保存先が設定されていません。",
            "action_ja": "保存先を選択してください。",
            "recoverable": true
        }))
        .expect("deserialize");
        assert_eq!(restored.code, EditorUserErrorCode::SaveDirectoryNotSet);
    }
}
