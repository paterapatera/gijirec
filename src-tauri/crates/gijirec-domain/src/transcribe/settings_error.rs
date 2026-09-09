//! Transcribe settings persistence errors.

use std::fmt;

/// Contract error codes for transcribe settings invoke responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TranscribeSettingsErrorCode {
    SettingsPersistFailed,
    InvalidModelVariant,
}

impl TranscribeSettingsErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SettingsPersistFailed => "SETTINGS_PERSIST_FAILED",
            Self::InvalidModelVariant => "INVALID_MODEL_VARIANT",
        }
    }
}

/// User-facing error payload per `docs/contracts/whisper-transcribe-settings.md`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscribeSettingsUserError {
    pub code: TranscribeSettingsErrorCode,
    pub message_ja: String,
    pub action_ja: String,
}

/// Issue observed while loading settings; caller logs and notifies without failing startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeSettingsLoadIssue {
    FileMissing,
    ParseError { detail: String },
}

impl TranscribeSettingsLoadIssue {
    pub fn message_ja(&self) -> &'static str {
        match self {
            Self::FileMissing => {
                "文字起こし設定ファイルが見つかりません。既定のモデルを使用します。"
            }
            Self::ParseError { .. } => {
                "文字起こし設定の読み込みに失敗しました。既定のモデルを使用します。"
            }
        }
    }
}

/// Successful load with optional non-fatal issue for notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscribeSettingsLoadResult {
    pub settings: super::settings::TranscribeSettings,
    pub issue: Option<TranscribeSettingsLoadIssue>,
}

/// Internal transcribe settings failures (save / validation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeSettingsError {
    SettingsPersistFailed { detail: String },
    InvalidModelVariant { detail: String },
}

impl TranscribeSettingsError {
    pub fn to_user_facing(&self) -> TranscribeSettingsUserError {
        match self {
            Self::SettingsPersistFailed { detail: _ } => TranscribeSettingsUserError {
                code: TranscribeSettingsErrorCode::SettingsPersistFailed,
                message_ja: "設定の保存に失敗しました".to_string(),
                action_ja: "アプリを再起動して再度お試しください".to_string(),
            },
            Self::InvalidModelVariant { detail: _ } => TranscribeSettingsUserError {
                code: TranscribeSettingsErrorCode::InvalidModelVariant,
                message_ja: "選択したモデルは利用できません".to_string(),
                action_ja: "一覧からモデルを選び直してください".to_string(),
            },
        }
    }
}

impl fmt::Display for TranscribeSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SettingsPersistFailed { detail } => {
                write!(f, "settings persist failed: {detail}")
            }
            Self::InvalidModelVariant { detail } => {
                write!(f, "invalid model variant: {detail}")
            }
        }
    }
}

impl std::error::Error for TranscribeSettingsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_failed_maps_to_contract_code() {
        let facing = TranscribeSettingsError::SettingsPersistFailed {
            detail: "disk full".to_string(),
        }
        .to_user_facing();
        assert_eq!(
            facing.code.as_str(),
            TranscribeSettingsErrorCode::SettingsPersistFailed.as_str()
        );
        assert!(!facing.message_ja.is_empty());
        assert!(!facing.action_ja.is_empty());
    }

    #[test]
    fn load_issue_provides_japanese_notification() {
        assert!(
            !TranscribeSettingsLoadIssue::FileMissing
                .message_ja()
                .is_empty()
        );
        assert!(
            !TranscribeSettingsLoadIssue::ParseError {
                detail: "bad json".to_string(),
            }
            .message_ja()
            .is_empty()
        );
    }
}
