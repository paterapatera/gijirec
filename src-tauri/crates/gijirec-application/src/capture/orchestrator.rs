//! Dual-capture lifecycle orchestration with no silent fallback.

use gijirec_domain::audio::{AudioDeviceId, CaptureError, CapturePhase, DeviceSelection};

macro_rules! define_capture_stream_port {
    ($(#[$meta:meta])* $trait_name:ident) => {
        $(#[$meta])*
        pub trait $trait_name: Send {
            fn open(&mut self) -> Result<(), CaptureError>;

            fn open_with_selection(
                &mut self,
                device_id: Option<&AudioDeviceId>,
            ) -> Result<(), CaptureError> {
                let _ = device_id;
                self.open()
            }

            fn close(&mut self);
            fn is_open(&self) -> bool;
        }
    };
}

define_capture_stream_port!(
    /// Port for opening/closing the microphone capture stream.
    MicCapturePort
);
define_capture_stream_port!(
    /// Port for opening/closing the system audio capture stream.
    SystemAudioCapturePort
);

/// Orchestrates mic + system audio capture lifecycle.
pub trait CaptureOrchestrator: Send {
    fn start(&mut self) -> Result<(), CaptureError>;
    fn start_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError>;
    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError>;
    fn stop(&mut self) -> Result<(), CaptureError>;
    fn phase(&self) -> CapturePhase;

    /// Safe stop when the active capture device disconnects during `capturing` (req 4.3).
    fn on_device_disconnected(&mut self) -> Result<(), CaptureError>;
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

    /// Handles device disconnect during capture (req 4.3).
    fn on_device_disconnected(&mut self) -> Result<(), CaptureError> {
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

    fn release_to_idle(&mut self) {
        self.force_stop_streams();
        if self.phase == CapturePhase::Capturing || self.phase == CapturePhase::Starting {
            let _ = self.set_phase(CapturePhase::Stopping);
        }
        self.phase = CapturePhase::Idle;
    }

    fn open_streams(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        if let Err(err) = self.mic.open_with_selection(selection.microphone_id()) {
            self.force_stop_streams();
            self.phase = CapturePhase::Error;
            return Err(err);
        }

        if let Err(err) = self.system.open_with_selection(selection.speaker_id()) {
            self.mic.close();
            self.phase = CapturePhase::Error;
            return Err(err);
        }

        Ok(())
    }
}

impl<M: MicCapturePort, S: SystemAudioCapturePort> CaptureOrchestrator
    for DefaultCaptureOrchestrator<M, S>
{
    fn phase(&self) -> CapturePhase {
        self.phase
    }

    fn start(&mut self) -> Result<(), CaptureError> {
        self.start_with_selection(&DeviceSelection::default())
    }

    fn start_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
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
                detail: "cannot start from error without restart_with_selection".to_string(),
            });
        }

        self.set_phase(CapturePhase::Starting)?;
        self.open_streams(selection)?;
        self.set_phase(CapturePhase::Capturing)?;
        Ok(())
    }

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        if self.phase == CapturePhase::Stopping {
            return Err(CaptureError::Internal {
                detail: "cannot restart while stopping".to_string(),
            });
        }

        if self.phase != CapturePhase::Idle {
            self.release_to_idle();
        }

        self.start_with_selection(selection)
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

    fn on_device_disconnected(&mut self) -> Result<(), CaptureError> {
        DefaultCaptureOrchestrator::on_device_disconnected(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_device_selection(device_id: Option<&AudioDeviceId>) -> Option<String> {
        device_id.map(|id| id.as_str().to_string())
    }

    fn close_open_capture_port(opened: &mut bool, close_count: &mut usize) {
        if *opened {
            *close_count += 1;
        }
        *opened = false;
    }

    struct MockMic {
        open_ok: bool,
        opened: bool,
        close_count: usize,
        last_selection: Option<String>,
    }

    impl MockMic {
        fn succeeds() -> Self {
            Self {
                open_ok: true,
                opened: false,
                close_count: 0,
                last_selection: None,
            }
        }

        fn fails() -> Self {
            Self {
                open_ok: false,
                opened: false,
                close_count: 0,
                last_selection: None,
            }
        }
    }

    macro_rules! impl_mock_capture_port_shell {
        ($trait:path, $ty:ty, |$self:ident, $device_id:ident| $open_with_selection:block) => {
            impl $trait for $ty {
                fn open(&mut self) -> Result<(), CaptureError> {
                    self.open_with_selection(None)
                }

                fn open_with_selection(
                    &mut self,
                    device_id: Option<&AudioDeviceId>,
                ) -> Result<(), CaptureError> {
                    let $self = self;
                    let $device_id = device_id;
                    $open_with_selection
                }

                fn close(&mut self) {
                    close_open_capture_port(&mut self.opened, &mut self.close_count);
                }

                fn is_open(&self) -> bool {
                    self.opened
                }
            }
        };
    }

    impl_mock_capture_port_shell!(MicCapturePort, MockMic, |mock, device_id| {
        mock.last_selection = record_device_selection(device_id);
        if mock.open_ok {
            mock.opened = true;
            Ok(())
        } else {
            Err(CaptureError::MicUnavailable)
        }
    });

    struct MockSystem {
        opens_before_success: usize,
        open_attempts: usize,
        opened: bool,
        close_count: usize,
        error: CaptureError,
        last_selection: Option<String>,
        fail_on_attempt: Option<usize>,
    }

    impl MockSystem {
        fn succeeds() -> Self {
            Self {
                opens_before_success: 0,
                open_attempts: 0,
                opened: false,
                close_count: 0,
                error: CaptureError::SystemAudioUnavailable,
                last_selection: None,
                fail_on_attempt: None,
            }
        }

        fn fails_with(error: CaptureError) -> Self {
            Self {
                opens_before_success: usize::MAX,
                open_attempts: 0,
                opened: false,
                close_count: 0,
                error,
                last_selection: None,
                fail_on_attempt: None,
            }
        }

        fn fails_first_open(error: CaptureError) -> Self {
            Self {
                opens_before_success: 1,
                open_attempts: 0,
                opened: false,
                close_count: 0,
                error,
                last_selection: None,
                fail_on_attempt: None,
            }
        }

        fn succeeds_except_on_attempt(attempt: usize, error: CaptureError) -> Self {
            Self {
                opens_before_success: 0,
                open_attempts: 0,
                opened: false,
                close_count: 0,
                error,
                last_selection: None,
                fail_on_attempt: Some(attempt),
            }
        }
    }

    impl_mock_capture_port_shell!(SystemAudioCapturePort, MockSystem, |system, device_id| {
        system.last_selection = record_device_selection(device_id);
        system.open_attempts += 1;
        if system.open_attempts <= system.opens_before_success {
            return Err(system.error.clone());
        }
        if system.fail_on_attempt == Some(system.open_attempts) {
            system.opened = false;
            return Err(system.error.clone());
        }
        system.opened = true;
        Ok(())
    });

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
    // req 4.2: 選択スピーカー失敗時にマイク単独で継続しない
    fn start_with_selection_speaker_failure_closes_mic_no_mic_only() {
        let mut orch = DefaultCaptureOrchestrator::new(
            MockMic::succeeds(),
            MockSystem::fails_with(CaptureError::SelectedSystemAudioUnavailable),
        );
        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-1".to_string()).expect("mic id")),
            Some(AudioDeviceId::new("spk-1".to_string()).expect("speaker id")),
        );

        let err = orch.start_with_selection(&selection).unwrap_err();
        assert_eq!(err, CaptureError::SelectedSystemAudioUnavailable);
        assert_eq!(orch.phase(), CapturePhase::Error);
        assert!(!orch.mic.is_open(), "mic must not remain open");
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

    /// Design unit test 5: `restart_with_selection` speaker failure must not leave mic-only capture.
    #[test]
    fn restart_with_selection_speaker_failure_closes_mic_no_mic_only() {
        let mut orch = DefaultCaptureOrchestrator::new(
            MockMic::succeeds(),
            MockSystem::succeeds_except_on_attempt(2, CaptureError::SelectedSystemAudioUnavailable),
        );
        let initial = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-1".to_string()).expect("mic id")),
            Some(AudioDeviceId::new("spk-1".to_string()).expect("speaker id")),
        );
        orch.start_with_selection(&initial)
            .expect("initial capture must reach capturing");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        assert!(orch.mic.is_open());
        assert!(orch.system.is_open());

        let changed = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-2".to_string()).expect("mic id")),
            Some(AudioDeviceId::new("spk-2".to_string()).expect("speaker id")),
        );
        let err = orch.restart_with_selection(&changed).unwrap_err();
        assert_eq!(err, CaptureError::SelectedSystemAudioUnavailable);
        assert_eq!(orch.phase(), CapturePhase::Error);
        assert!(
            !orch.mic.is_open(),
            "mic must not remain open after restart speaker failure"
        );
        assert!(!orch.system.is_open());
        assert_ne!(
            orch.phase(),
            CapturePhase::Capturing,
            "must not fall back to mic-only capture on restart"
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

    /// Design unit test 6: `restart_with_selection` recovers from error to capturing.
    #[test]
    fn restart_from_error_recovers_to_capturing() {
        let mut orch = DefaultCaptureOrchestrator::new(
            MockMic::succeeds(),
            MockSystem::fails_first_open(CaptureError::SelectedSystemAudioUnavailable),
        );

        orch.start_with_selection(&DeviceSelection::default())
            .expect_err("first start fails");
        assert_eq!(orch.phase(), CapturePhase::Error);

        orch.restart_with_selection(&DeviceSelection::default())
            .expect("restart after error");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        assert!(orch.mic.is_open());
        assert!(orch.system.is_open());
    }

    /// Design unit test 6 (restart path): error after failed restart, then recover via `restart_with_selection`.
    #[test]
    fn restart_with_selection_recovers_after_failed_restart() {
        let mut orch = DefaultCaptureOrchestrator::new(
            MockMic::succeeds(),
            MockSystem::succeeds_except_on_attempt(2, CaptureError::SelectedSystemAudioUnavailable),
        );

        orch.start_with_selection(&DeviceSelection::default())
            .expect("initial start");
        assert_eq!(orch.phase(), CapturePhase::Capturing);

        let bad = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-bad-restart".to_string()).expect("mic id")),
            Some(AudioDeviceId::new("spk-bad-restart".to_string()).expect("speaker id")),
        );
        let err = orch.restart_with_selection(&bad).unwrap_err();
        assert_eq!(err, CaptureError::SelectedSystemAudioUnavailable);
        assert_eq!(orch.phase(), CapturePhase::Error);
        assert!(!orch.mic.is_open());

        orch.restart_with_selection(&DeviceSelection::default())
            .expect("restart after failed restart");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        assert!(orch.mic.is_open());
        assert!(orch.system.is_open());
    }

    #[test]
    fn start_with_selection_passes_device_ids_to_ports() {
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::succeeds(), MockSystem::succeeds());
        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-a".to_string()).expect("mic id")),
            Some(AudioDeviceId::new("spk-b".to_string()).expect("speaker id")),
        );

        orch.start_with_selection(&selection).expect("start");
        assert_eq!(orch.mic.last_selection.as_deref(), Some("mic-a"));
        assert_eq!(orch.system.last_selection.as_deref(), Some("spk-b"));
    }

    #[test]
    fn start_uses_os_default_when_selection_is_default() {
        let mut orch = DefaultCaptureOrchestrator::new(MockMic::succeeds(), MockSystem::succeeds());

        orch.start().expect("start");
        assert_eq!(orch.mic.last_selection, None);
        assert_eq!(orch.system.last_selection, None);
    }
}
