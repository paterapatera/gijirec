//! Composition root: orchestrator, pipeline hold, and lifecycle wiring helpers.

use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
};

use crate::capture_ports::CaptureStreamHandles;
use crate::capture_processing::CapturePipelineState;

/// Fully composed capture stack ready for Tauri lifecycle injection.
pub(crate) struct ComposedCapture {
    pub orchestrator: Box<dyn CaptureOrchestrator>,
    pub pipeline: CapturePipelineState,
}

/// Builds the production capture stack with platform adapters.
pub(crate) fn build_capture_stack() -> ComposedCapture {
    let (streams, mic, system) = CaptureStreamHandles::new_pair();
    compose_with_ports(mic, system, streams)
}

/// Builds a capture stack from injectable ports (unit tests and composition root).
pub(crate) fn compose_with_ports<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    let orchestrator = Box::new(DefaultCaptureOrchestrator::new(mic, system));
    let pipeline = CapturePipelineState::new(streams);
    ComposedCapture {
        orchestrator,
        pipeline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, Mutex};

    struct TrackingMic {
        order: Arc<AtomicU8>,
    }

    struct TrackingSystem {
        order: Arc<AtomicU8>,
    }

    impl MicCapturePort for TrackingMic {
        fn open(&mut self) -> Result<(), CaptureError> {
            self.order.store(1, Ordering::SeqCst);
            Ok(())
        }

        fn close(&mut self) {
            self.order.store(0, Ordering::SeqCst);
        }

        fn is_open(&self) -> bool {
            self.order.load(Ordering::SeqCst) >= 1
        }
    }

    impl SystemAudioCapturePort for TrackingSystem {
        fn open(&mut self) -> Result<(), CaptureError> {
            let mic_first = self.order.load(Ordering::SeqCst) == 1;
            if !mic_first {
                return Err(CaptureError::Internal {
                    detail: "system opened before mic".to_string(),
                });
            }
            self.order.store(2, Ordering::SeqCst);
            Ok(())
        }

        fn close(&mut self) {
            self.order.store(0, Ordering::SeqCst);
        }

        fn is_open(&self) -> bool {
            self.order.load(Ordering::SeqCst) == 2
        }
    }

    #[test]
    fn compose_opens_mic_before_system_without_silent_fallback() {
        let order = Arc::new(AtomicU8::new(0));
        let (streams, _, _) = CaptureStreamHandles::new_pair();
        let mut composed = compose_with_ports(
            TrackingMic {
                order: Arc::clone(&order),
            },
            TrackingSystem {
                order: Arc::clone(&order),
            },
            streams,
        );

        composed.orchestrator.start().expect("start");
        assert_eq!(composed.orchestrator.phase(), CapturePhase::Capturing);
        assert_eq!(order.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn compose_system_failure_closes_mic() {
        struct FailingSystem;

        impl SystemAudioCapturePort for FailingSystem {
            fn open(&mut self) -> Result<(), CaptureError> {
                Err(CaptureError::SystemAudioUnavailable)
            }

            fn close(&mut self) {}

            fn is_open(&self) -> bool {
                false
            }
        }

        struct OpenFlag(Arc<Mutex<bool>>);

        impl MicCapturePort for OpenFlag {
            fn open(&mut self) -> Result<(), CaptureError> {
                *self.0.lock().expect("lock") = true;
                Ok(())
            }

            fn close(&mut self) {
                *self.0.lock().expect("lock") = false;
            }

            fn is_open(&self) -> bool {
                *self.0.lock().expect("lock")
            }
        }

        let mic_open = Arc::new(Mutex::new(false));
        let (streams, _, _) = CaptureStreamHandles::new_pair();
        let mut composed =
            compose_with_ports(OpenFlag(Arc::clone(&mic_open)), FailingSystem, streams);

        let err = composed.orchestrator.start().unwrap_err();
        assert_eq!(err, CaptureError::SystemAudioUnavailable);
        assert_eq!(composed.orchestrator.phase(), CapturePhase::Error);
        assert!(
            !*mic_open.lock().expect("lock"),
            "mic must close on system failure"
        );
    }

    #[test]
    fn compose_holds_pipeline_components() {
        let (streams, _, _) = CaptureStreamHandles::new_pair();
        let composed = compose_with_ports(
            TrackingMic {
                order: Arc::new(AtomicU8::new(0)),
            },
            TrackingSystem {
                order: Arc::new(AtomicU8::new(0)),
            },
            streams,
        );
        assert_eq!(
            composed
                .pipeline
                .chunk_emitter
                .lock()
                .expect("lock")
                .next_sequence(),
            0
        );
        assert_eq!(composed.pipeline.pcm_bus.buffer_drops_total(), 0);
    }

    #[cfg(not(target_os = "linux"))]
    mod integration {
        use super::*;
        use crate::capture_ports::{CaptureStreamHandles, SyntheticMicPort, SyntheticSystemPort};
        use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
        use std::sync::{Arc, Mutex};

        // Integration Test 3: start → capturing → stop → idle でストリームと処理スレッドを解放 (req 3.4)
        #[test]
        fn start_capturing_stop_idle_releases_streams_and_processing_thread() {
            let mic_open = Arc::new(Mutex::new(false));
            let sys_open = Arc::new(Mutex::new(false));
            let (streams, _, _) = CaptureStreamHandles::new_pair();
            let composed = compose_with_ports(
                SyntheticMicPort::new(streams.clone(), Arc::clone(&mic_open)),
                SyntheticSystemPort::new(streams.clone(), Arc::clone(&sys_open)),
                streams,
            );

            let mut orch = composed.orchestrator;
            let pipeline = composed.pipeline;

            orch.start().expect("start");
            assert_eq!(orch.phase(), CapturePhase::Capturing);
            assert!(*mic_open.lock().expect("lock"));
            assert!(*sys_open.lock().expect("lock"));

            pipeline.on_capture_started();
            assert!(
                pipeline.processing_is_active(),
                "processing thread must run while capturing"
            );

            pipeline.on_capture_stopping();
            orch.stop().expect("stop");

            assert_eq!(orch.phase(), CapturePhase::Idle);
            assert!(
                !*mic_open.lock().expect("lock"),
                "mic stream handle must be released"
            );
            assert!(
                !*sys_open.lock().expect("lock"),
                "system stream handle must be released"
            );
            assert!(
                !pipeline.processing_is_active(),
                "processing thread must stop after shutdown"
            );
        }
    }

    #[cfg(target_os = "windows")]
    mod windows_hardware {
        use super::*;

        /// Skip reason for mic + WASAPI loopback dual-start (Integration Test 1, req 1.2, 6.2).
        const WINDOWS_DUAL_CAPTURE_HARDWARE_SKIP: &str =
            "CI: requires Windows mic permission and default WASAPI loopback output; run with --ignored on local hardware";

        #[test]
        fn documents_windows_dual_capture_hardware_ci_skip_reason() {
            let reason = WINDOWS_DUAL_CAPTURE_HARDWARE_SKIP;
            assert!(reason.contains("WASAPI"));
            assert!(reason.contains("loopback"));
            assert!(reason.contains("CI"));
        }

        // Integration Test 1: mic + WASAPI loopback simultaneous start → capturing (req 1.2, 6.2)
        #[test]
        #[ignore = "CI: requires Windows mic permission and default WASAPI loopback output; run with --ignored on local hardware"]
        fn integration_mic_and_wasapi_loopback_reach_capturing_on_hardware() {
            let mut composed = build_capture_stack();
            composed.orchestrator.start().expect("mic+loopback start");
            assert_eq!(composed.orchestrator.phase(), CapturePhase::Capturing);
            assert!(
                composed.pipeline.streams.mic_sample_rate_hz().is_some(),
                "mic stream must be open"
            );
            assert!(
                composed.pipeline.streams.system_sample_rate_hz().is_some(),
                "WASAPI loopback stream must be open"
            );
            composed.orchestrator.stop().expect("stop");
            assert_eq!(composed.orchestrator.phase(), CapturePhase::Idle);
        }
    }

    #[cfg(target_os = "macos")]
    mod macos_hardware {
        use super::*;

        /// Skip reason for mic + ScreenCaptureKit dual-start (Integration Test 2, req 1.2, 6.1, 7.1).
        const MACOS_DUAL_CAPTURE_HARDWARE_SKIP: &str =
            "CI: requires macOS mic permission and ScreenCaptureKit screen recording permission; run with --ignored on local hardware";

        #[test]
        fn documents_macos_dual_capture_hardware_ci_skip_reason() {
            let reason = MACOS_DUAL_CAPTURE_HARDWARE_SKIP;
            assert!(reason.contains("ScreenCaptureKit"));
            assert!(reason.contains("permission"));
            assert!(reason.contains("CI"));
        }

        // Integration Test 2: mic + ScreenCaptureKit simultaneous start → capturing (req 1.2, 6.1, 7.1)
        #[test]
        #[ignore = "CI: requires macOS mic permission and ScreenCaptureKit screen recording permission; run with --ignored on local hardware"]
        fn integration_mic_and_sck_reach_capturing_on_hardware() {
            let mut composed = build_capture_stack();
            composed.orchestrator.start().expect("mic+sck start");
            assert_eq!(composed.orchestrator.phase(), CapturePhase::Capturing);
            assert!(
                composed.pipeline.streams.mic_sample_rate_hz().is_some(),
                "mic stream must be open"
            );
            assert!(
                composed.pipeline.streams.system_sample_rate_hz().is_some(),
                "ScreenCaptureKit system audio stream must be open"
            );
            composed.orchestrator.stop().expect("stop");
            assert_eq!(composed.orchestrator.phase(), CapturePhase::Idle);
        }
    }
}
