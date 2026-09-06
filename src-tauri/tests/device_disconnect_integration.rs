//! Req 4.3: stream disconnect during capturing → Error phase + DEVICE_DISCONNECTED emit.

use gijirec_lib::test_support::{
    CapturePipelineState, SyntheticMicPort, SyntheticSystemPort, new_pipeline, new_stream_handles,
    notify_stream_disconnected, start_processing,
};
use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator,
};
use gijirec_presentation::application::device_selection::{
    DeviceSelectionError, DeviceSelectionService,
};
use gijirec_presentation::domain::audio::{AudioDeviceList, CapturePhase, DeviceSelection};
use gijirec_presentation::tauri::events::RecordingEventEmitter;
use gijirec_presentation::tauri::lifecycle::{
    CaptureLifecycleState, CapturePlatformSupport, RecordingUnsupportedPlatformNotifier,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct SupportedPlatform;

impl CapturePlatformSupport for SupportedPlatform {
    fn is_capture_supported(&self) -> bool {
        true
    }
}

struct NoopDeviceSelection;

impl DeviceSelectionService for NoopDeviceSelection {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
        Ok(AudioDeviceList {
            inputs: vec![],
            outputs: vec![],
        })
    }

    fn get_selection(&self) -> DeviceSelection {
        DeviceSelection::default()
    }

    fn set_selection(
        &self,
        selection: DeviceSelection,
    ) -> Result<DeviceSelection, DeviceSelectionError> {
        Ok(selection)
    }

    fn set_ui_visible(&self, _visible: bool) {}
}

struct DisconnectStack {
    streams: gijirec_lib::test_support::CaptureStreamHandles,
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pipeline: Arc<CapturePipelineState>,
    emitter: Arc<RecordingEventEmitter>,
    mic_close_count: Arc<AtomicUsize>,
}

impl DisconnectStack {
    fn new() -> Self {
        let mic_opened = Arc::new(Mutex::new(false));
        let sys_opened = Arc::new(Mutex::new(false));
        let mic_close_count = Arc::new(AtomicUsize::new(0));
        let sys_close_count = Arc::new(AtomicUsize::new(0));

        let streams = new_stream_handles();
        let mic_port = SyntheticMicPort::new_instrumented(
            streams.clone(),
            Arc::clone(&mic_opened),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&mic_close_count),
            Arc::new(Mutex::new(None)),
        );
        let sys_port = SyntheticSystemPort::new_instrumented(
            streams.clone(),
            Arc::clone(&sys_opened),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&sys_close_count),
            Arc::new(Mutex::new(None)),
        );

        let pipeline = Arc::new(new_pipeline(streams.clone()));
        let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> = Arc::new(Mutex::new(
            DefaultCaptureOrchestrator::new(mic_port, sys_port),
        ));

        let lifecycle = Arc::new(CaptureLifecycleState::new(
            Arc::clone(&orchestrator),
            Arc::new(NoopDeviceSelection),
            Arc::new(SupportedPlatform),
            Arc::new(RecordingUnsupportedPlatformNotifier::new()),
        ));
        let emitter = Arc::new(RecordingEventEmitter::new());
        lifecycle.init_emitter(Arc::clone(&emitter)
            as Arc<dyn gijirec_presentation::tauri::events::CaptureEventEmitter>);
        lifecycle.set_processing_hook(Arc::clone(&pipeline)
            as Arc<dyn gijirec_presentation::tauri::lifecycle::CaptureProcessingHook>);

        let lifecycle_for_stream = Arc::clone(&lifecycle);
        pipeline.set_stream_disconnect_handler(Arc::new(move || {
            lifecycle_for_stream.handle_stream_disconnected();
        }));

        Self {
            streams,
            orchestrator,
            pipeline,
            emitter,
            mic_close_count,
        }
    }

    fn start_capturing(&self) {
        let mut orch = self.orchestrator.lock().expect("lock");
        orch.start_with_selection(&DeviceSelection::default())
            .expect("initial start");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        drop(orch);
        start_processing(&self.pipeline);
        assert!(self.pipeline.processing_is_active());
    }
}

/// Integration Test (req 4.3): runtime stream disconnect → safe stop + DEVICE_DISCONNECTED.
#[test]
fn integration_stream_disconnect_emits_device_disconnected() {
    let stack = DisconnectStack::new();
    stack.start_capturing();

    notify_stream_disconnected(&stack.streams);

    assert_eq!(
        stack.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Error,
        "orchestrator must enter error phase after stream disconnect"
    );
    assert!(
        !stack.pipeline.processing_is_active(),
        "processing thread must stop after disconnect"
    );
    assert!(
        stack.mic_close_count.load(Ordering::SeqCst) >= 1,
        "mic stream must be closed on disconnect"
    );

    let errors = stack.emitter.errors();
    assert_eq!(
        errors.len(),
        1,
        "expected one capture error event: {errors:?}"
    );
    assert_eq!(errors[0].code, "DEVICE_DISCONNECTED");
    assert!(!errors[0].action_ja.is_empty());
    assert!(!errors[0].message_ja.is_empty());

    let phases = stack.emitter.phases();
    assert!(
        phases.iter().any(|payload| payload.phase == "error"),
        "expected error phase event: {phases:?}"
    );
}
