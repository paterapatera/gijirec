//! Transcribe lifecycle phase and legal state transitions.

use std::fmt;

/// Lifecycle phase per `docs/contracts/whisper-transcribe-status.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscribePhase {
    Idle,
    LoadingModel,
    Ready,
    Transcribing,
    Stopping,
    Error,
}

impl TranscribePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::LoadingModel => "loading_model",
            Self::Ready => "ready",
            Self::Transcribing => "transcribing",
            Self::Stopping => "stopping",
            Self::Error => "error",
        }
    }

    /// Returns whether `target` is a legal next phase from `self`.
    pub fn can_transition_to(self, target: Self) -> bool {
        self != target && is_legal_transition(self, target)
    }

    /// Returns `target` when the transition is legal per the design state diagram.
    pub fn try_transition_to(self, target: Self) -> Result<Self, PhaseTransitionError> {
        if !self.can_transition_to(target) {
            return Err(PhaseTransitionError {
                from: self,
                to: target,
            });
        }
        Ok(target)
    }
}

/// Rejects transitions that are not allowed by the transcribe lifecycle state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTransitionError {
    pub from: TranscribePhase,
    pub to: TranscribePhase,
}

impl fmt::Display for PhaseTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "illegal transcribe phase transition: {} -> {}",
            self.from.as_str(),
            self.to.as_str(),
        )
    }
}

impl std::error::Error for PhaseTransitionError {}

fn is_legal_transition(from: TranscribePhase, to: TranscribePhase) -> bool {
    matches!(
        (from, to),
        (TranscribePhase::Idle, TranscribePhase::LoadingModel)
            | (TranscribePhase::LoadingModel, TranscribePhase::Ready)
            | (TranscribePhase::LoadingModel, TranscribePhase::Error)
            | (TranscribePhase::Ready, TranscribePhase::Transcribing)
            | (TranscribePhase::Transcribing, TranscribePhase::Ready)
            | (TranscribePhase::Transcribing, TranscribePhase::Stopping)
            | (TranscribePhase::Transcribing, TranscribePhase::Error)
            | (TranscribePhase::Error, TranscribePhase::LoadingModel)
            | (TranscribePhase::Stopping, TranscribePhase::Idle)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [TranscribePhase; 6] = [
        TranscribePhase::Idle,
        TranscribePhase::LoadingModel,
        TranscribePhase::Ready,
        TranscribePhase::Transcribing,
        TranscribePhase::Stopping,
        TranscribePhase::Error,
    ];

    const LEGAL: [(TranscribePhase, TranscribePhase); 9] = [
        (TranscribePhase::Idle, TranscribePhase::LoadingModel),
        (TranscribePhase::LoadingModel, TranscribePhase::Ready),
        (TranscribePhase::LoadingModel, TranscribePhase::Error),
        (TranscribePhase::Ready, TranscribePhase::Transcribing),
        (TranscribePhase::Transcribing, TranscribePhase::Ready),
        (TranscribePhase::Transcribing, TranscribePhase::Stopping),
        (TranscribePhase::Transcribing, TranscribePhase::Error),
        (TranscribePhase::Error, TranscribePhase::LoadingModel),
        (TranscribePhase::Stopping, TranscribePhase::Idle),
    ];

    #[test]
    fn contract_phase_strings() {
        assert_eq!(TranscribePhase::Idle.as_str(), "idle");
        assert_eq!(TranscribePhase::LoadingModel.as_str(), "loading_model");
        assert_eq!(TranscribePhase::Ready.as_str(), "ready");
        assert_eq!(TranscribePhase::Transcribing.as_str(), "transcribing");
        assert_eq!(TranscribePhase::Stopping.as_str(), "stopping");
        assert_eq!(TranscribePhase::Error.as_str(), "error");
    }

    #[test]
    fn serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_string(&TranscribePhase::LoadingModel).unwrap(),
            "\"loading_model\""
        );
        assert_eq!(
            serde_json::from_str::<TranscribePhase>("\"transcribing\"").unwrap(),
            TranscribePhase::Transcribing
        );
    }

    #[test]
    fn allows_design_legal_transitions() {
        for (from, to) in LEGAL {
            assert!(from.can_transition_to(to));
            assert_eq!(from.try_transition_to(to), Ok(to));
        }
    }

    #[test]
    fn rejects_all_illegal_transitions() {
        for from in ALL {
            for to in ALL {
                check_transition(from, to);
            }
        }
    }

    fn check_transition(from: TranscribePhase, to: TranscribePhase) {
        let legal = LEGAL.contains(&(from, to));
        assert_eq!(from.can_transition_to(to), legal);
        if legal {
            assert_eq!(from.try_transition_to(to), Ok(to));
        } else {
            assert_eq!(
                from.try_transition_to(to),
                Err(PhaseTransitionError { from, to })
            );
        }
    }

    #[test]
    fn rejects_self_transition() {
        for phase in ALL {
            assert!(!phase.can_transition_to(phase));
            assert!(
                phase
                    .try_transition_to(phase)
                    .unwrap_err()
                    .to_string()
                    .contains("illegal transcribe phase transition")
            );
        }
    }

    #[test]
    fn rejects_idle_to_transcribing() {
        assert!(!TranscribePhase::Idle.can_transition_to(TranscribePhase::Transcribing));
    }

    #[test]
    fn rejects_ready_to_error() {
        assert!(!TranscribePhase::Ready.can_transition_to(TranscribePhase::Error));
    }

    #[test]
    fn rejects_stopping_to_ready() {
        assert!(!TranscribePhase::Stopping.can_transition_to(TranscribePhase::Ready));
    }

    #[test]
    fn rejects_removed_shutdown_shortcuts() {
        assert!(!TranscribePhase::Idle.can_transition_to(TranscribePhase::Ready));
        assert!(!TranscribePhase::Idle.can_transition_to(TranscribePhase::Stopping));
        assert!(!TranscribePhase::Ready.can_transition_to(TranscribePhase::Stopping));
        assert!(!TranscribePhase::Error.can_transition_to(TranscribePhase::Stopping));
    }
}
