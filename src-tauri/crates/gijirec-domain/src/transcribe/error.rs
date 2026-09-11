//! Internal transcribe errors and user-facing error mapping.

use std::fmt;

/// Contract error codes for `whisper-transcribe://error` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TranscribeErrorCode {
    ModelDownloadFailed,
    ModelCorrupt,
    ModelNotFound,
    InferenceFailed,
    UpstreamCaptureError,
    Internal,
}

impl TranscribeErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelDownloadFailed => "MODEL_DOWNLOAD_FAILED",
            Self::ModelCorrupt => "MODEL_CORRUPT",
            Self::ModelNotFound => "MODEL_NOT_FOUND",
            Self::InferenceFailed => "INFERENCE_FAILED",
            Self::UpstreamCaptureError => "UPSTREAM_CAPTURE_ERROR",
            Self::Internal => "INTERNAL",
        }
    }
}

/// User-facing error payload per `docs/contracts/whisper-transcribe-status.md`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UserFacingTranscribeError {
    pub code: TranscribeErrorCode,
    pub message_ja: String,
    pub action_ja: String,
    pub recoverable: bool,
}

impl UserFacingTranscribeError {
    pub fn action_ja_is_present(&self) -> bool {
        !self.action_ja.trim().is_empty()
    }
}

/// Internal transcribe failure causes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeError {
    ModelDownloadFailed { detail: String },
    ModelCorrupt { detail: String },
    ModelNotFound { detail: String },
    InferenceFailed { detail: String },
    UpstreamCaptureError,
    Internal { detail: String },
}

impl TranscribeError {
    pub fn to_user_facing(&self) -> UserFacingTranscribeError {
        match self {
            Self::ModelDownloadFailed { detail: _ } => UserFacingTranscribeError {
                code: TranscribeErrorCode::ModelDownloadFailed,
                message_ja: "音声認識モデルの取得に失敗しました。".to_string(),
                action_ja: "ネットワーク接続を確認し、アプリを再起動して再試行してください"
                    .to_string(),
                recoverable: true,
            },
            Self::ModelCorrupt { detail: _ } => UserFacingTranscribeError {
                code: TranscribeErrorCode::ModelCorrupt,
                message_ja: "ローカルの音声認識モデルが破損しているか読み込めません。".to_string(),
                action_ja: "設定からモデルを再取得してください".to_string(),
                recoverable: true,
            },
            Self::ModelNotFound { detail: _ } => UserFacingTranscribeError {
                code: TranscribeErrorCode::ModelNotFound,
                message_ja: "音声認識モデルが見つかりません。".to_string(),
                action_ja: "アプリを再起動し、モデル取得を完了してください".to_string(),
                recoverable: true,
            },
            Self::InferenceFailed { detail: _ } => UserFacingTranscribeError {
                code: TranscribeErrorCode::InferenceFailed,
                message_ja: "音声認識処理中に回復不能なエラーが発生しました。".to_string(),
                action_ja:
                    "アプリを再起動してください。改善しない場合はモデル再取得を試してください"
                        .to_string(),
                recoverable: false,
            },
            Self::UpstreamCaptureError => UserFacingTranscribeError {
                code: TranscribeErrorCode::UpstreamCaptureError,
                message_ja: "音声キャプチャでエラーが発生したため、文字起こしを一時停止しました。"
                    .to_string(),
                action_ja: "キャプチャエラーを解消後、文字起こしは自動再開します".to_string(),
                recoverable: true,
            },
            Self::Internal { detail: _ } => UserFacingTranscribeError {
                code: TranscribeErrorCode::Internal,
                message_ja: "予期しないエラーが発生しました。".to_string(),
                action_ja: "アプリを再起動してください。改善しない場合はログを共有してください"
                    .to_string(),
                recoverable: false,
            },
        }
    }
}

impl fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelDownloadFailed { detail } => {
                write!(f, "model download failed: {detail}")
            }
            Self::ModelCorrupt { detail } => write!(f, "model corrupt: {detail}"),
            Self::ModelNotFound { detail } => write!(f, "model not found: {detail}"),
            Self::InferenceFailed { detail } => write!(f, "inference failed: {detail}"),
            Self::UpstreamCaptureError => write!(f, "upstream capture error"),
            Self::Internal { detail } => write!(f, "internal transcribe error: {detail}"),
        }
    }
}

impl std::error::Error for TranscribeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_facing_contract_tests::{
        assert_all_errors_serde_round_trip, assert_contract_error_mappings,
        assert_serde_produces_contract_json_shape,
    };

    fn all_errors() -> Vec<TranscribeError> {
        vec![
            TranscribeError::ModelDownloadFailed {
                detail: "network timeout".to_string(),
            },
            TranscribeError::ModelCorrupt {
                detail: "checksum mismatch".to_string(),
            },
            TranscribeError::ModelNotFound {
                detail: "offline first launch".to_string(),
            },
            TranscribeError::InferenceFailed {
                detail: "whisper context error".to_string(),
            },
            TranscribeError::UpstreamCaptureError,
            TranscribeError::Internal {
                detail: "worker join timeout".to_string(),
            },
        ]
    }

    #[test]
    fn maps_all_transcribe_errors_to_contract_codes() {
        let expected = [
            "MODEL_DOWNLOAD_FAILED",
            "MODEL_CORRUPT",
            "MODEL_NOT_FOUND",
            "INFERENCE_FAILED",
            "UPSTREAM_CAPTURE_ERROR",
            "INTERNAL",
        ];

        assert_contract_error_mappings(all_errors(), &expected, TranscribeError::to_user_facing);
    }

    #[test]
    fn user_facing_payload_does_not_leak_internal_detail() {
        let detail = "secret-internal-stack-trace";
        for error in [
            TranscribeError::ModelDownloadFailed {
                detail: detail.to_string(),
            },
            TranscribeError::ModelCorrupt {
                detail: detail.to_string(),
            },
            TranscribeError::ModelNotFound {
                detail: detail.to_string(),
            },
            TranscribeError::InferenceFailed {
                detail: detail.to_string(),
            },
            TranscribeError::Internal {
                detail: detail.to_string(),
            },
        ] {
            let facing = error.to_user_facing();
            assert!(
                !facing.message_ja.contains(detail),
                "message_ja must not contain internal detail"
            );
            assert!(
                !facing.action_ja.contains(detail),
                "action_ja must not contain internal detail"
            );
        }
    }

    #[test]
    fn inference_and_internal_errors_are_not_recoverable() {
        assert!(
            !TranscribeError::InferenceFailed {
                detail: "boom".to_string(),
            }
            .to_user_facing()
            .recoverable
        );
        assert!(
            !TranscribeError::Internal {
                detail: "boom".to_string(),
            }
            .to_user_facing()
            .recoverable
        );
    }

    #[test]
    fn model_and_upstream_errors_are_recoverable() {
        for error in [
            TranscribeError::ModelDownloadFailed {
                detail: "x".to_string(),
            },
            TranscribeError::ModelCorrupt {
                detail: "x".to_string(),
            },
            TranscribeError::ModelNotFound {
                detail: "x".to_string(),
            },
            TranscribeError::UpstreamCaptureError,
        ] {
            assert!(error.to_user_facing().recoverable);
        }
    }

    #[test]
    fn serde_produces_contract_json_shape() {
        let facing = TranscribeError::ModelDownloadFailed {
            detail: "ignored".to_string(),
        }
        .to_user_facing();
        assert_serde_produces_contract_json_shape(&facing, "MODEL_DOWNLOAD_FAILED");
    }

    #[test]
    fn serde_round_trips_all_contract_codes() {
        assert_all_errors_serde_round_trip(all_errors(), TranscribeError::to_user_facing);
    }

    #[test]
    fn display_includes_detail_for_model_download_failure() {
        let err = TranscribeError::ModelDownloadFailed {
            detail: "connection reset".to_string(),
        };
        assert!(err.to_string().contains("connection reset"));
    }
}
