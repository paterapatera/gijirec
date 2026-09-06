//! Internal capture errors and user-facing error mapping.

use std::fmt;

/// Contract error codes for `audio-capture://error` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserFacingErrorCode {
    MicUnavailable,
    MicPermissionDenied,
    SystemAudioUnavailable,
    SystemAudioPermissionDenied,
    DeviceDisconnected,
    SelectedMicUnavailable,
    SelectedSystemAudioUnavailable,
    MacosOutputNotDefault,
    Internal,
}

impl UserFacingErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MicUnavailable => "MIC_UNAVAILABLE",
            Self::MicPermissionDenied => "MIC_PERMISSION_DENIED",
            Self::SystemAudioUnavailable => "SYSTEM_AUDIO_UNAVAILABLE",
            Self::SystemAudioPermissionDenied => "SYSTEM_AUDIO_PERMISSION_DENIED",
            Self::DeviceDisconnected => "DEVICE_DISCONNECTED",
            Self::SelectedMicUnavailable => "SELECTED_MIC_UNAVAILABLE",
            Self::SelectedSystemAudioUnavailable => "SELECTED_SYSTEM_AUDIO_UNAVAILABLE",
            Self::MacosOutputNotDefault => "MACOS_OUTPUT_NOT_DEFAULT",
            Self::Internal => "INTERNAL",
        }
    }
}

/// User-facing error payload per `docs/contracts/audio-capture-status.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserFacingError {
    pub code: UserFacingErrorCode,
    pub message_ja: String,
    pub action_ja: String,
    pub recoverable: bool,
}

impl UserFacingError {
    pub fn action_ja_is_present(&self) -> bool {
        !self.action_ja.trim().is_empty()
    }
}

/// Internal capture failure causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    MicUnavailable,
    MicPermissionDenied,
    SystemAudioUnavailable,
    SystemAudioPermissionDenied,
    DeviceDisconnected,
    SelectedMicUnavailable,
    SelectedSystemAudioUnavailable,
    MacosOutputNotDefault,
    Internal { detail: String },
}

impl CaptureError {
    pub fn to_user_facing(self) -> UserFacingError {
        match self {
            Self::MicUnavailable => UserFacingError {
                code: UserFacingErrorCode::MicUnavailable,
                message_ja: "マイク入力を利用できません。".to_string(),
                action_ja: "マイク接続とシステム設定を確認してください。".to_string(),
                recoverable: true,
            },
            Self::MicPermissionDenied => UserFacingError {
                code: UserFacingErrorCode::MicPermissionDenied,
                message_ja: "マイクの使用が許可されていません。".to_string(),
                action_ja: "設定 → プライバシー → マイクで gijirec を許可してください。"
                    .to_string(),
                recoverable: true,
            },
            Self::SystemAudioUnavailable => UserFacingError {
                code: UserFacingErrorCode::SystemAudioUnavailable,
                message_ja: "システム音声を取得できません。".to_string(),
                action_ja: "出力デバイスと OS バージョンを確認してください。".to_string(),
                recoverable: true,
            },
            Self::SystemAudioPermissionDenied => UserFacingError {
                code: UserFacingErrorCode::SystemAudioPermissionDenied,
                message_ja: "システム音声の取得が許可されていません。".to_string(),
                action_ja: "設定 → プライバシー → 画面とシステムオーディオ録音で許可してください。"
                    .to_string(),
                recoverable: true,
            },
            Self::DeviceDisconnected => UserFacingError {
                code: UserFacingErrorCode::DeviceDisconnected,
                message_ja: "音声デバイスが切断されました。".to_string(),
                action_ja: "デバイスを再接続してアプリを再起動してください。".to_string(),
                recoverable: true,
            },
            Self::SelectedMicUnavailable => UserFacingError {
                code: UserFacingErrorCode::SelectedMicUnavailable,
                message_ja: "選択したマイクが利用できません。".to_string(),
                action_ja: "別のマイクを選ぶか、接続とマイク権限を確認してください".to_string(),
                recoverable: true,
            },
            Self::SelectedSystemAudioUnavailable => UserFacingError {
                code: UserFacingErrorCode::SelectedSystemAudioUnavailable,
                message_ja: "選択したスピーカーが利用できません。".to_string(),
                action_ja: "別のスピーカーを選ぶか、出力デバイスと権限を確認してください"
                    .to_string(),
                recoverable: true,
            },
            Self::MacosOutputNotDefault => UserFacingError {
                code: UserFacingErrorCode::MacosOutputNotDefault,
                message_ja: "選択したスピーカーがシステムの出力先になっていません。".to_string(),
                action_ja:
                    "システム設定 → サウンドで出力先を変更するか、現在の出力先を選んでください"
                        .to_string(),
                recoverable: true,
            },
            Self::Internal { detail: _ } => UserFacingError {
                code: UserFacingErrorCode::Internal,
                message_ja: "予期しないエラーが発生しました。".to_string(),
                action_ja: "アプリを再起動してください。改善しない場合はログを共有してください。"
                    .to_string(),
                recoverable: false,
            },
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MicUnavailable => write!(f, "microphone unavailable"),
            Self::MicPermissionDenied => write!(f, "microphone permission denied"),
            Self::SystemAudioUnavailable => write!(f, "system audio unavailable"),
            Self::SystemAudioPermissionDenied => write!(f, "system audio permission denied"),
            Self::DeviceDisconnected => write!(f, "audio device disconnected"),
            Self::SelectedMicUnavailable => write!(f, "selected microphone unavailable"),
            Self::SelectedSystemAudioUnavailable => {
                write!(f, "selected system audio output unavailable")
            }
            Self::MacosOutputNotDefault => {
                write!(f, "macos selected output is not system default")
            }
            Self::Internal { detail } => write!(f, "internal capture error: {detail}"),
        }
    }
}

impl std::error::Error for CaptureError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_errors() -> Vec<CaptureError> {
        vec![
            CaptureError::MicUnavailable,
            CaptureError::MicPermissionDenied,
            CaptureError::SystemAudioUnavailable,
            CaptureError::SystemAudioPermissionDenied,
            CaptureError::DeviceDisconnected,
            CaptureError::SelectedMicUnavailable,
            CaptureError::SelectedSystemAudioUnavailable,
            CaptureError::MacosOutputNotDefault,
            CaptureError::Internal {
                detail: "test".to_string(),
            },
        ]
    }

    #[test]
    // Testing Strategy 5: 各 CaptureError が契約 code / action_ja にマップ (req 5.4, audio-capture-status.md)
    fn maps_all_capture_errors_to_contract_codes() {
        let expected = [
            "MIC_UNAVAILABLE",
            "MIC_PERMISSION_DENIED",
            "SYSTEM_AUDIO_UNAVAILABLE",
            "SYSTEM_AUDIO_PERMISSION_DENIED",
            "DEVICE_DISCONNECTED",
            "SELECTED_MIC_UNAVAILABLE",
            "SELECTED_SYSTEM_AUDIO_UNAVAILABLE",
            "MACOS_OUTPUT_NOT_DEFAULT",
            "INTERNAL",
        ];

        let errors = all_errors();
        assert_eq!(
            errors.len(),
            expected.len(),
            "every contract error code must have a CaptureError mapping"
        );

        for (error, code) in errors.into_iter().zip(expected) {
            let facing = error.to_user_facing();
            assert_eq!(
                facing.code.as_str(),
                code,
                "unexpected contract code mapping"
            );
            assert!(
                facing.action_ja_is_present(),
                "action_ja must be non-empty for contract code {code}"
            );
            assert!(
                !facing.message_ja.trim().is_empty(),
                "message_ja must be non-empty for contract code {code}"
            );
        }
    }

    #[test]
    // Testing Strategy 5: INTERNAL は recoverable=false、権限系は recoverable=true
    fn internal_error_is_not_recoverable() {
        let facing = CaptureError::Internal {
            detail: "boom".to_string(),
        }
        .to_user_facing();
        assert!(!facing.recoverable);
    }

    #[test]
    // Testing Strategy 5: 権限拒否エラーは利用者が回復可能
    fn permission_errors_are_recoverable() {
        for error in [
            CaptureError::MicPermissionDenied,
            CaptureError::SystemAudioPermissionDenied,
        ] {
            assert!(error.to_user_facing().recoverable);
        }
    }

    #[test]
    // audio-device-selection 4.1–4.2: 選択デバイス文脈エラーは契約 action_ja を含み回復可能
    fn selected_device_errors_map_to_contract_payloads() {
        let cases = [
            (
                CaptureError::SelectedMicUnavailable,
                "SELECTED_MIC_UNAVAILABLE",
                "別のマイクを選ぶか、接続とマイク権限を確認してください",
            ),
            (
                CaptureError::SelectedSystemAudioUnavailable,
                "SELECTED_SYSTEM_AUDIO_UNAVAILABLE",
                "別のスピーカーを選ぶか、出力デバイスと権限を確認してください",
            ),
            (
                CaptureError::MacosOutputNotDefault,
                "MACOS_OUTPUT_NOT_DEFAULT",
                "システム設定 → サウンドで出力先を変更するか、現在の出力先を選んでください",
            ),
        ];

        for (error, code, action_ja) in cases {
            let facing = error.to_user_facing();
            assert_eq!(facing.code.as_str(), code);
            assert_eq!(facing.action_ja, action_ja);
            assert!(facing.recoverable);
            assert!(!facing.message_ja.trim().is_empty());
        }
    }
}
