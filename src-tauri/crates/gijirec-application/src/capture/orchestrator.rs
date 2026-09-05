//! Dual-capture lifecycle orchestration with no silent fallback.

use gijirec_domain::audio::{CaptureError, CapturePhase};

/// Port for opening/closing the microphone capture stream.
pub trait MicCapturePort: Send {
    fn open(&mut self) -> Result<(), CaptureError>;
    fn close(&mut self);
    fn is_open(&self) -> bool;
}

/// Port for opening/closing the system audio capture stream.
pub trait SystemAudioCapturePort: Send {
    fn open(&mut self) -> Result<(), CaptureError>;
    fn close(&mut self);
    fn is_open(&self) -> bool;
}

/// Orchestrates mic + system audio capture lifecycle.
pub trait CaptureOrchestrator: Send {
    fn start(&mut self) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<(), CaptureError>;
    fn phase(&self) -> CapturePhase;
}

/// Default orchestrator: mic first, system second; no silent mic-only fallback.
pub struct DefaultCaptureOrchestrator<M, S> {
    phase: CapturePhase,
    mic: M,
    system: S,
}

impl<M: MicCapturePort, S: SystemAudioCapturePort> DefaultCaptureOrchestrator<M, S> {
    pub fn new(mic: M, system: S) -> Self {
        Self {
            phase: CapturePhase::Idle,
            mic,
            system,
        }
    }

    fn set_phase(&mut self, next: CapturePhase) -> Result<(), CaptureError> {
        self.phase = self
            .phase
            .transition_to(next)
            .map_err(|_| CaptureError::Internal {
                detail: format!(
                    "illegal phase transition: {} -> {}",
                    self.phase.as_str(),
                    next.as_str()
                ),
            })?;
        Ok(())
    }

    /// Handles device disconnect during capture (5.3).
    pub fn on_device_disconnected(&mut self) -> Result<(), CaptureError> {
        if self.phase != CapturePhase::Capturing {
            return Ok(());
        }
        let err = CaptureError::DeviceDisconnected;
        self.force_stop_streams();
        self.phase = CapturePhase::Error;
        Err(err)
    }

    fn force_stop_streams(&mut self) {
        self.system.close();
        self.mic.close();
    }
}

impl<M: MicCapturePort, S: SystemAudioCapturePort> CaptureOrchestrator
    for DefaultCaptureOrchestrator<M, S>
{
    fn phase(&self) -> CapturePhase {
        self.phase
    }

    fn start(&mut self) -> Result<(), CaptureError> {
        if self.phase == CapturePhase::Capturing {
            return Ok(());
        }
        if self.phase == CapturePhase::Starting {
            return Ok(());
        }
        if self.phase == CapturePhase::Stopping {
            return Err(CaptureError::Internal {
                detail: "cannot start while stopping".to_string(),
            });
        }
        if self.phase == CapturePhase::Error {
            return Err(CaptureError::Internal {
                detail: "cannot start from error without stop".to_string(),
            });
        }

        self.set_phase(CapturePhase::Starting)?;

        if let Err(err) = self.mic.open() {
            self.force_stop_streams();
            self.phase = CapturePhase::Error;
            return Err(err);
        }

        if let Err(err) = self.system.open() {
            self.mic.close();
            self.phase = CapturePhase::Error;
            return Err(err);
        }

        self.set_phase(CapturePhase::Capturing)?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        if self.phase == CapturePhase::Idle {
            return Ok(());
        }
        if self.phase == CapturePhase::Stopping {
            return Ok(());
        }

        let _ = self.set_phase(CapturePhase::Stopping);
        self.force_stop_streams();
        self.phase = CapturePhase::Idle;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMic {
        open_ok: bool,
        opened: bool,
        close_count: usize,
    }

    impl MockMic {
        fn succeeds() -> Self {
            Self {
                open_ok: true,
                opened: false,
                close_count: 0,
            }
        }

        fn fails() -> Self {
            Self {
                open_ok: false,
                opened: false,
                close_count: 0,
            }
        }
    }

    impl MicCapturePort for MockMic {
        fn open(&mut self) -> Result<(), CaptureError> {
            if self.open_ok {
                self.opened = true;
                Ok(())
            } else {
                Err(CaptureError::MicUnavailable)
            }
        }

        fn close(&mut self) {
            if self.opened {
                self.close_count += 1;
            }
            self.opened = false;
        }

        fn is_open(&self) -> bool {
            self.opened
        }
    }

    struct MockSystem {
        open_ok: bool,
        opened: bool,
        close_count: usize,
        error: CaptureError,
    }

    impl MockSystem {
        fn succeeds() -> Self {
            Self {
                open_ok: true,
                opened: false,
                close_count: 0,
                error: CaptureError::SystemAudioUnavailable,
            }
        }

        fn fails_with(error: CaptureError) -> Self {
            Self {
                open_ok: false,
                opened: false,
                close_count: 0,
                error,
            }
        }
    }

    impl SystemAudioCapturePort for MockSystem {
        fn open(&mut self) -> Result<(), CaptureError> {
            if self.open_ok {
                self.opened = true;
                Ok(())
            } else {
                Err(self.error.clone())
            }
        }

        fn close(&mut self) {
            if self.opened {
                self.close_count += 1;
            }
            self.opened = false;
        }

        fn is_open(&self) -> bool {
            self.opened
        }
    }

    #[test]
    fn start_to_capturing_to_stop_to_idle() {
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::succeeds(), MockSystem::succeeds());
        assert_eq!(orch.phase(), CapturePhase::Idle);

        orch.start().expect("start");
        assert_eq!(orch.phase(), CapturePhase::Capturing);

        orch.stop().expect("stop");
        assert_eq!(orch.phase(), CapturePhase::Idle);
    }

    #[test]
    // Testing Strategy 4: システム音声失敗時にマイク単独で継続しない (req 1.3, 5.2)
    fn system_failure_closes_mic_and_does_not_reach_capturing() {
        let mic = MockMic::succeeds();
        let mut orch = DefaultCaptureOrchestrator::new(
            mic,
            MockSystem::fails_with(CaptureError::SystemAudioUnavailable),
        );

        let err = orch.start().unwrap_err();
        assert_eq!(err, CaptureError::SystemAudioUnavailable);
        assert_eq!(orch.phase(), CapturePhase::Error);
        assert_ne!(
            orch.phase(),
            CapturePhase::Capturing,
            "must not fall back to mic-only capture"
        );
        assert!(!orch.mic.is_open(), "mic must be closed on system failure");
        assert!(!orch.system.is_open());
    }

    #[test]
    // Testing Strategy 4: マイク失敗時にシステム単独で継続しない (req 1.3, 5.2)
    fn mic_failure_does_not_open_system() {
        let system = MockSystem::succeeds();
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::fails(), system);

        let err = orch.start().unwrap_err();
        assert_eq!(err, CaptureError::MicUnavailable);
        assert_eq!(orch.phase(), CapturePhase::Error);
        assert_ne!(
            orch.phase(),
            CapturePhase::Capturing,
            "must not fall back to system-only capture"
        );
        assert!(
            !orch.system.is_open(),
            "system must not open when mic fails"
        );
    }

    #[test]
    fn stop_is_idempotent_from_idle() {
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::succeeds(), MockSystem::succeeds());
        orch.stop().expect("idle stop");
        orch.stop().expect("idle stop again");
        assert_eq!(orch.phase(), CapturePhase::Idle);
    }

    #[test]
    fn start_is_idempotent_while_capturing() {
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::succeeds(), MockSystem::succeeds());
        orch.start().expect("start");
        orch.start().expect("start again");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
    }

    #[test]
    fn device_disconnect_moves_to_error_and_closes_streams() {
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::succeeds(), MockSystem::succeeds());
        orch.start().expect("start");
        assert!(orch.mic.is_open());
        assert!(orch.system.is_open());

        let err = orch.on_device_disconnected().unwrap_err();
        assert_eq!(err, CaptureError::DeviceDisconnected);
        assert_eq!(orch.phase(), CapturePhase::Error);
        assert!(!orch.mic.is_open());
        assert!(!orch.system.is_open());
    }

    #[test]
    fn stop_from_error_releases_resources() {
        let mut orch = DefaultCaptureOrchestrator::new(
            MockMic::succeeds(),
            MockSystem::fails_with(CaptureError::SystemAudioPermissionDenied),
        );
        orch.start().expect_err("system fails");
        assert_eq!(orch.phase(), CapturePhase::Error);

        orch.stop().expect("stop from error");
        assert_eq!(orch.phase(), CapturePhase::Idle);
        assert!(!orch.mic.is_open());
    }
}
