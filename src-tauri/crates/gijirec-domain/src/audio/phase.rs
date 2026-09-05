//! Capture lifecycle phase and legal state transitions.

use std::fmt;

/// Lifecycle phase per `docs/contracts/audio-capture-status.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapturePhase {
    Idle,
    Starting,
    Capturing,
    Stopping,
    Error,
}

impl CapturePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Capturing => "capturing",
            Self::Stopping => "stopping",
            Self::Error => "error",
        }
    }

    /// Returns the next phase when the transition is legal per the design state diagram.
    pub fn transition_to(self, next: Self) -> Result<Self, PhaseTransitionError> {
        if self == next || !is_legal_transition(self, next) {
            return Err(PhaseTransitionError {
                from: self,
                to: next,
            });
        }
        Ok(next)
    }
}

/// Rejects transitions that are not allowed by the capture lifecycle state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTransitionError {
    pub from: CapturePhase,
    pub to: CapturePhase,
}

impl fmt::Display for PhaseTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "illegal capture phase transition: {} -> {}",
            self.from.as_str(),
            self.to.as_str(),
        )
    }
}

impl std::error::Error for PhaseTransitionError {}

fn is_legal_transition(from: CapturePhase, to: CapturePhase) -> bool {
    matches!(
        (from, to),
        (CapturePhase::Idle, CapturePhase::Starting)
            | (CapturePhase::Starting, CapturePhase::Capturing)
            | (CapturePhase::Starting, CapturePhase::Error)
            | (CapturePhase::Starting, CapturePhase::Stopping)
            | (CapturePhase::Capturing, CapturePhase::Stopping)
            | (CapturePhase::Capturing, CapturePhase::Error)
            | (CapturePhase::Error, CapturePhase::Idle)
            | (CapturePhase::Error, CapturePhase::Stopping)
            | (CapturePhase::Stopping, CapturePhase::Idle)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGAL: [(CapturePhase, CapturePhase); 9] = [
        (CapturePhase::Idle, CapturePhase::Starting),
        (CapturePhase::Starting, CapturePhase::Capturing),
        (CapturePhase::Starting, CapturePhase::Error),
        (CapturePhase::Starting, CapturePhase::Stopping),
        (CapturePhase::Capturing, CapturePhase::Stopping),
        (CapturePhase::Capturing, CapturePhase::Error),
        (CapturePhase::Error, CapturePhase::Idle),
        (CapturePhase::Error, CapturePhase::Stopping),
        (CapturePhase::Stopping, CapturePhase::Idle),
    ];

    #[test]
    fn contract_phase_strings() {
        assert_eq!(CapturePhase::Idle.as_str(), "idle");
        assert_eq!(CapturePhase::Starting.as_str(), "starting");
        assert_eq!(CapturePhase::Capturing.as_str(), "capturing");
        assert_eq!(CapturePhase::Stopping.as_str(), "stopping");
        assert_eq!(CapturePhase::Error.as_str(), "error");
    }

    #[test]
    fn allows_design_legal_transitions() {
        for (from, to) in LEGAL {
            assert_eq!(from.transition_to(to), Ok(to));
        }
    }

    #[test]
    fn rejects_idle_to_capturing() {
        let err = CapturePhase::Idle
            .transition_to(CapturePhase::Capturing)
            .unwrap_err();
        assert_eq!(
            err,
            PhaseTransitionError {
                from: CapturePhase::Idle,
                to: CapturePhase::Capturing,
            }
        );
    }

    #[test]
    fn rejects_capturing_to_starting() {
        assert!(
            CapturePhase::Capturing
                .transition_to(CapturePhase::Starting)
                .is_err()
        );
    }

    #[test]
    fn rejects_self_transition() {
        assert!(
            CapturePhase::Capturing
                .transition_to(CapturePhase::Capturing)
                .is_err()
        );
    }

    #[test]
    fn rejects_stopping_to_capturing() {
        assert!(
            CapturePhase::Stopping
                .transition_to(CapturePhase::Capturing)
                .is_err()
        );
    }
}
