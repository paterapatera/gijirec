//! Capture session IPC error codes.

/// Contract error codes for capture session commands per `docs/contracts/capture-session-toggle.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureSessionErrorCode {
    TransitionBusy,
    CaptureStartFailed,
    UnsupportedPlatform,
    Internal,
}

impl CaptureSessionErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TransitionBusy => "TRANSITION_BUSY",
            Self::CaptureStartFailed => "CAPTURE_START_FAILED",
            Self::UnsupportedPlatform => "UNSUPPORTED_PLATFORM",
            Self::Internal => "INTERNAL",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureSessionErrorCode;

    #[test]
    fn contract_error_code_strings() {
        assert_eq!(
            CaptureSessionErrorCode::TransitionBusy.as_str(),
            "TRANSITION_BUSY"
        );
        assert_eq!(
            CaptureSessionErrorCode::CaptureStartFailed.as_str(),
            "CAPTURE_START_FAILED"
        );
        assert_eq!(
            CaptureSessionErrorCode::UnsupportedPlatform.as_str(),
            "UNSUPPORTED_PLATFORM"
        );
        assert_eq!(CaptureSessionErrorCode::Internal.as_str(), "INTERNAL");
    }

    #[test]
    fn serde_uses_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&CaptureSessionErrorCode::TransitionBusy).unwrap(),
            "\"TRANSITION_BUSY\""
        );
    }
}
