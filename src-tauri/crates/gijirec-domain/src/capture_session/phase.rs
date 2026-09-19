//! Capture session lifecycle phase (user-facing start state).

use std::fmt;

/// User session phase per `docs/contracts/capture-session-toggle.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSessionPhase {
    Idle,
    Starting,
    Active,
}

impl CaptureSessionPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Active => "active",
        }
    }
}

impl fmt::Display for CaptureSessionPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureSessionPhase;

    #[test]
    fn contract_phase_strings() {
        assert_eq!(CaptureSessionPhase::Idle.as_str(), "idle");
        assert_eq!(CaptureSessionPhase::Starting.as_str(), "starting");
        assert_eq!(CaptureSessionPhase::Active.as_str(), "active");
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(CaptureSessionPhase::Idle.to_string(), "idle");
        assert_eq!(CaptureSessionPhase::Active.to_string(), "active");
    }

    #[test]
    fn serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_string(&CaptureSessionPhase::Starting).unwrap(),
            "\"starting\""
        );
        assert_eq!(
            serde_json::from_str::<CaptureSessionPhase>("\"active\"").unwrap(),
            CaptureSessionPhase::Active
        );
        assert_eq!(
            serde_json::from_str::<CaptureSessionPhase>("\"idle\"").unwrap(),
            CaptureSessionPhase::Idle
        );
    }
}
