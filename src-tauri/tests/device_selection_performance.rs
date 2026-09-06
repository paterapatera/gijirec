//! Performance/Load 1 (audio-device-selection req 5.3):
//! selection change → `capturing` recovery under 2 s with mock ports (no hardware).

use gijirec_lib::test_support::{
    CapturePipelineState, SyntheticMicPort, SyntheticSystemPort, new_pipeline, new_stream_handles,
    start_processing,
};
use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator,
};
use gijirec_presentation::application::device_selection::{
    CaptureSelectionPort, DefaultDeviceSelectionService, DeviceEnumeratorPort,
    DeviceSelectionError, DeviceSelectionEvents, DeviceSelectionService, NoopSpeakerPreflight,
    RecordingDeviceSelectionObservability, SystemClock,
};
use gijirec_presentation::domain::audio::{
    AudioDeviceId, AudioDeviceInfo, AudioDeviceKind, AudioDeviceList, CaptureError, CapturePhase,
    DeviceSelection,
};
use gijirec_presentation::tauri::device_selection::set_device_selection_impl;
use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Design Performance/Load 1 budget (req 5.3).
const RESTART_BUDGET_MS: u128 = 2_000;

fn mic(id: &str, default: bool) -> AudioDeviceInfo {
    AudioDeviceInfo::new(
        AudioDeviceId::new(id.to_string()).expect("id"),
        format!("Mic {id}"),
        AudioDeviceKind::Input,
        default,
    )
}

fn speaker(id: &str, default: bool) -> AudioDeviceInfo {
    AudioDeviceInfo::new(
        AudioDeviceId::new(id.to_string()).expect("id"),
        format!("Speaker {id}"),
        AudioDeviceKind::Output,
        default,
    )
}

fn sample_device_list() -> AudioDeviceList {
    AudioDeviceList {
        inputs: vec![mic("mic-default", true), mic("mic-usb", false)],
        outputs: vec![speaker("spk-default", true), speaker("spk-hdmi", false)],
    }
}

struct MockEnumerator {
    list: AudioDeviceList,
}

impl DeviceEnumeratorPort for MockEnumerator {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
        Ok(self.list.clone())
    }
}

struct NoopEvents;

impl DeviceSelectionEvents for NoopEvents {
    fn emit_selection_changed(&self, _selection: &DeviceSelection) {}
    fn emit_devices_changed(&self, _devices: &AudioDeviceList, _timestamp_ms: u64) {}
}

struct RecaptureCaptureSelectionAdapter {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pipeline: Arc<CapturePipelineState>,
}

impl CaptureSelectionPort for RecaptureCaptureSelectionAdapter {
    fn capture_phase(&self) -> CapturePhase {
        self.orchestrator.lock().expect("lock").phase()
    }

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.pipeline.stop_processing_for_recapture();
        let result = self
            .orchestrator
            .lock()
            .expect("lock")
            .restart_with_selection(selection);
        if result.is_ok() {
            start_processing(&self.pipeline);
        }
        result
    }
}

struct PerformanceStack {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    device_selection: Arc<dyn DeviceSelectionService>,
    pipeline: Arc<CapturePipelineState>,
}

impl PerformanceStack {
    fn new(observability: Arc<RecordingDeviceSelectionObservability>) -> Self {
        let mic_opened = Arc::new(Mutex::new(false));
        let sys_opened = Arc::new(Mutex::new(false));
        let mic_open_count = Arc::new(AtomicUsize::new(0));
        let mic_close_count = Arc::new(AtomicUsize::new(0));
        let sys_open_count = Arc::new(AtomicUsize::new(0));
        let sys_close_count = Arc::new(AtomicUsize::new(0));
        let mic_prod = Arc::new(Mutex::new(None));
        let sys_prod = Arc::new(Mutex::new(None));

        let streams = new_stream_handles();
        let mic_port = SyntheticMicPort::new_instrumented(
            streams.clone(),
            Arc::clone(&mic_opened),
            Arc::clone(&mic_open_count),
            Arc::clone(&mic_close_count),
            Arc::clone(&mic_prod),
        );
        let sys_port = SyntheticSystemPort::new_instrumented(
            streams.clone(),
            Arc::clone(&sys_opened),
            Arc::clone(&sys_open_count),
            Arc::clone(&sys_close_count),
            Arc::clone(&sys_prod),
        );

        let pipeline = Arc::new(new_pipeline(streams));
        let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> = Arc::new(Mutex::new(
            DefaultCaptureOrchestrator::new(mic_port, sys_port),
        ));

        let obs = Arc::clone(&observability);
        let device_selection: Arc<dyn DeviceSelectionService> =
            Arc::new(DefaultDeviceSelectionService::new(
                gijirec_presentation::application::device_selection::DeviceSelectionStore::new(),
                MockEnumerator {
                    list: sample_device_list(),
                },
                RecaptureCaptureSelectionAdapter {
                    orchestrator: Arc::clone(&orchestrator),
                    pipeline: Arc::clone(&pipeline),
                },
                NoopSpeakerPreflight,
                NoopEvents,
                SystemClock,
                obs,
            ));

        Self {
            orchestrator,
            device_selection,
            pipeline,
        }
    }

    fn start_capturing(&self) {
        let mut orch = self.orchestrator.lock().expect("lock");
        orch.start_with_selection(&DeviceSelection::default())
            .expect("initial start");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        drop(orch);
        self.pipeline.on_capture_started();
        assert!(self.pipeline.processing_is_active());
    }
}

/// Performance/Load 1 (req 5.3): mock-port stack measures restart and asserts < 2 s.
#[test]
fn performance_selection_restart_under_two_seconds_mock_ports() {
    let observability = Arc::new(RecordingDeviceSelectionObservability::new());
    let stack = PerformanceStack::new(Arc::clone(&observability));
    stack.start_capturing();

    let new_selection = DeviceSelection::new(
        Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
        Some(AudioDeviceId::new("spk-hdmi".to_string()).expect("id")),
    );

    let wall_started = Instant::now();
    let applied = set_device_selection_impl(stack.device_selection.as_ref(), new_selection.clone())
        .expect("selection change while capturing");
    assert_eq!(applied, new_selection);

    assert_eq!(
        stack.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Capturing,
        "phase must recover to capturing"
    );
    assert!(
        stack.pipeline.processing_is_active(),
        "processing thread must restart after recapture"
    );
    let wall_elapsed_ms = wall_started.elapsed().as_millis();

    let started = observability.recapture_started.lock().expect("lock");
    assert_eq!(started.len(), 1, "recapture must be logged once");
    drop(started);

    let completed = observability.recapture_completed.lock().expect("lock");
    assert_eq!(
        completed.len(),
        1,
        "recapture completion must be logged once"
    );
    let tracing_duration_ms = completed[0].1;
    drop(completed);

    eprintln!(
        "PERF device_selection_restart_duration_ms={tracing_duration_ms} wall_elapsed_ms={wall_elapsed_ms}"
    );

    assert!(
        u128::from(tracing_duration_ms) < RESTART_BUDGET_MS,
        "device_selection_restart_duration_ms={tracing_duration_ms} must be < {RESTART_BUDGET_MS} ms (req 5.3)"
    );
    assert!(
        wall_elapsed_ms < RESTART_BUDGET_MS,
        "wall_elapsed_ms={wall_elapsed_ms} must be < {RESTART_BUDGET_MS} ms (req 5.3)"
    );
}
