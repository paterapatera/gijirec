//! Internal editor errors and user-facing error mapping.

use std::fmt;

/// Contract error codes for editor invoke responses (`docs/contracts/transcript-editor-status.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EditorUserErrorCode {
    SaveDirectoryNotSet,
    SaveDirectoryUnavailable,
    SaveDirectoryCreateFailed,
    SaveFileWriteFailed,
    SavePartialFailure,
    SettingsPersistFailed,
    Internal,
}

impl EditorUserErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SaveDirectoryNotSet => "SAVE_DIRECTORY_NOT_SET",
            Self::SaveDirectoryUnavailable => "SAVE_DIRECTORY_UNAVAILABLE",
            Self::SaveDirectoryCreateFailed => "SAVE_DIRECTORY_CREATE_FAILED",
            Self::SaveFileWriteFailed => "SAVE_FILE_WRITE_FAILED",
            Self::SavePartialFailure => "SAVE_PARTIAL_FAILURE",
            Self::SettingsPersistFailed => "SETTINGS_PERSIST_FAILED",
            Self::Internal => "INTERNAL",
        }
    }
}

/// User-facing error payload per `docs/contracts/transcript-editor-status.md`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EditorUserError {
    pub code: EditorUserErrorCode,
    pub message_ja: String,
    pub action_ja: String,
    pub recoverable: bool,
}

impl EditorUserError {
    pub fn action_ja_is_present(&self) -> bool {
        !self.action_ja.trim().is_empty()
    }
}

/// Internal editor failure causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    SaveDirectoryNotSet,
    SaveDirectoryUnavailable { detail: String },
    SaveDirectoryCreateFailed { detail: String },
    SaveFileWriteFailed { path: String, detail: String },
    SavePartialFailure { detail: String },
    SettingsPersistFailed { detail: String },
    Internal { detail: String },
}

impl EditorError {
    pub fn to_user_facing(&self) -> EditorUserError {
        match self {
            Self::SaveDirectoryNotSet => EditorUserError {
                code: EditorUserErrorCode::SaveDirectoryNotSet,
                message_ja: "保存先ディレクトリが設定されていません。".to_string(),
                action_ja: "保存先を選択してください".to_string(),
                recoverable: true,
            },
            Self::SaveDirectoryUnavailable { detail: _ } => EditorUserError {
                code: EditorUserErrorCode::SaveDirectoryUnavailable,
                message_ja: "保存先ディレクトリにアクセスできません。".to_string(),
                action_ja: "別の保存先ディレクトリを選択してください".to_string(),
                recoverable: true,
            },
            Self::SaveDirectoryCreateFailed { detail: _ } => EditorUserError {
                code: EditorUserErrorCode::SaveDirectoryCreateFailed,
                message_ja: "保存用のサブディレクトリを作成できませんでした。".to_string(),
                action_ja: "別の保存先を選択するか、フォルダの書き込み権限を確認してください"
                    .to_string(),
                recoverable: true,
            },
            Self::SaveFileWriteFailed { path: _, detail: _ } => EditorUserError {
                code: EditorUserErrorCode::SaveFileWriteFailed,
                message_ja: "ファイルの書き込みに失敗しました。".to_string(),
                action_ja: "保存を再試行するか、別の保存先を選択してください".to_string(),
                recoverable: true,
            },
            Self::SavePartialFailure { detail: _ } => EditorUserError {
                code: EditorUserErrorCode::SavePartialFailure,
                message_ja: "一部のファイルの保存に失敗しました。".to_string(),
                action_ja: "保存結果を確認し、失敗したファイルを再保存してください".to_string(),
                recoverable: true,
            },
            Self::SettingsPersistFailed { detail: _ } => EditorUserError {
                code: EditorUserErrorCode::SettingsPersistFailed,
                message_ja: "設定の保存に失敗しました。".to_string(),
                action_ja: "ディスクの空き容量とフォルダの書き込み権限を確認してください"
                    .to_string(),
                recoverable: true,
            },
            Self::Internal { detail: _ } => EditorUserError {
                code: EditorUserErrorCode::Internal,
                message_ja: "予期しないエラーが発生しました。".to_string(),
                action_ja: "アプリを再起動してください。改善しない場合はログを共有してください"
                    .to_string(),
                recoverable: false,
            },
        }
    }
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SaveDirectoryNotSet => write!(f, "save directory not set"),
            Self::SaveDirectoryUnavailable { detail } => {
                write!(f, "save directory unavailable: {detail}")
            }
            Self::SaveDirectoryCreateFailed { detail } => {
                write!(f, "save directory create failed: {detail}")
            }
            Self::SaveFileWriteFailed { path, detail } => {
                write!(f, "save file write failed ({path}): {detail}")
            }
            Self::SavePartialFailure { detail } => write!(f, "save partial failure: {detail}"),
            Self::SettingsPersistFailed { detail } => {
                write!(f, "settings persist failed: {detail}")
            }
            Self::Internal { detail } => write!(f, "internal editor error: {detail}"),
        }
    }
}

impl std::error::Error for EditorError {}

#[cfg(test)]
mod tests {
    use super::{EditorError, EditorUserError, EditorUserErrorCode};

    fn all_errors() -> Vec<EditorError> {
        vec![
            EditorError::SaveDirectoryNotSet,
            EditorError::SaveDirectoryUnavailable {
                detail: "path not found".to_string(),
            },
            EditorError::SaveDirectoryCreateFailed {
                detail: "access denied".to_string(),
            },
            EditorError::SaveFileWriteFailed {
                path: "handwriting.md".to_string(),
                detail: "disk full".to_string(),
            },
            EditorError::SavePartialFailure {
                detail: "1 of 3 files failed".to_string(),
            },
            EditorError::SettingsPersistFailed {
                detail: "permission denied".to_string(),
            },
            EditorError::Internal {
                detail: "unexpected panic".to_string(),
            },
        ]
    }

    #[test]
    fn maps_all_editor_errors_to_contract_codes() {
        let expected = [
            "SAVE_DIRECTORY_NOT_SET",
            "SAVE_DIRECTORY_UNAVAILABLE",
            "SAVE_DIRECTORY_CREATE_FAILED",
            "SAVE_FILE_WRITE_FAILED",
            "SAVE_PARTIAL_FAILURE",
            "SETTINGS_PERSIST_FAILED",
            "INTERNAL",
        ];

        let errors = all_errors();
        assert_eq!(errors.len(), expected.len());

        for (error, code) in errors.iter().zip(expected) {
            let facing = error.to_user_facing();
            assert_eq!(facing.code.as_str(), code);
            assert!(
                facing.action_ja_is_present(),
                "action_ja must be non-empty for {code}"
            );
            assert!(
                !facing.message_ja.trim().is_empty(),
                "message_ja must be non-empty for {code}"
            );
        }
    }

    #[test]
    fn user_facing_payload_does_not_leak_internal_detail_or_document_bodies() {
        let secret = "secret-internal-stack-trace";
        let document_body = "これは手動議事録の全文です。";
        for error in [
            EditorError::SaveDirectoryUnavailable {
                detail: secret.to_string(),
            },
            EditorError::SaveDirectoryCreateFailed {
                detail: secret.to_string(),
            },
            EditorError::SaveFileWriteFailed {
                path: "handwriting.md".to_string(),
                detail: format!("{secret} {document_body}"),
            },
            EditorError::SavePartialFailure {
                detail: secret.to_string(),
            },
            EditorError::SettingsPersistFailed {
                detail: secret.to_string(),
            },
            EditorError::Internal {
                detail: secret.to_string(),
            },
        ] {
            let facing = error.to_user_facing();
            assert!(!facing.message_ja.contains(secret));
            assert!(!facing.action_ja.contains(secret));
            assert!(!facing.message_ja.contains(document_body));
            assert!(!facing.action_ja.contains(document_body));
        }
    }

    #[test]
    fn internal_error_is_not_recoverable() {
        assert!(
            !EditorError::Internal {
                detail: "boom".to_string(),
            }
            .to_user_facing()
            .recoverable
        );
    }

    #[test]
    fn user_errors_are_recoverable_except_internal() {
        for error in all_errors() {
            let recoverable = error.to_user_facing().recoverable;
            if matches!(error, EditorError::Internal { .. }) {
                assert!(!recoverable);
            } else {
                assert!(recoverable, "expected recoverable for {error:?}");
            }
        }
    }

    #[test]
    fn serde_produces_contract_json_shape() {
        let facing = EditorError::SaveDirectoryNotSet.to_user_facing();
        let json = serde_json::to_value(&facing).expect("serialize user-facing error");
        let obj = json.as_object().expect("object payload");
        assert_eq!(
            obj.get("code").and_then(|v| v.as_str()),
            Some("SAVE_DIRECTORY_NOT_SET")
        );
        assert!(obj.get("message_ja").and_then(|v| v.as_str()).is_some());
        assert!(obj.get("action_ja").and_then(|v| v.as_str()).is_some());
        assert_eq!(obj.get("recoverable").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(obj.len(), 4, "payload must contain exactly four fields");
    }

    #[test]
    fn serde_round_trips_all_contract_codes() {
        for error in all_errors() {
            let facing = error.to_user_facing();
            let json = serde_json::to_string(&facing).expect("serialize");
            let restored: EditorUserError = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(facing, restored);
        }
    }

    #[test]
    fn partial_and_full_write_failures_map_to_distinct_codes() {
        assert_eq!(
            EditorError::SaveFileWriteFailed {
                path: "handwriting.md".to_string(),
                detail: "io error".to_string(),
            }
            .to_user_facing()
            .code,
            EditorUserErrorCode::SaveFileWriteFailed
        );
        assert_eq!(
            EditorError::SavePartialFailure {
                detail: "ai-transcription.jsonl failed".to_string(),
            }
            .to_user_facing()
            .code,
            EditorUserErrorCode::SavePartialFailure
        );
    }
}
