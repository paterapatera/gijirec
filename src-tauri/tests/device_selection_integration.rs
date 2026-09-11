//! Integration Tests 1–4 (audio-device-selection): selection change → capturing recovery,
//! SELECTED_MIC_UNAVAILABLE emission, UI-visible-only hotplug, and PCM sequence continuity.

use gijirec_lib::test_support::{
    CapturePipelineState, SyntheticMicPort, SyntheticSystemPort, new_pipeline, new_stream_handles,
    processing_is_active, start_processing, stop_processing_for_recapture,
};
use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
};
use gijirec_presentation::application::device_selection::{
    CaptureSelectionPort, DefaultDeviceSelectionService, DeviceEnumeratorPort,
    DeviceSelectionClock, DeviceSelectionError, DeviceSelectionEvents, DeviceSelectionService,
    HOTPLUG_POLL_INTERVAL_MS, NoopDeviceSelectionObservability, NoopSpeakerPreflight, SystemClock,
};
use gijirec_presentation::domain::audio::pcm_chunk::{
    CHUNK_FRAME_COUNT, PcmChunk, PcmChunkConsumer, PcmConsumerError,
};
use gijirec_presentation::domain::audio::{
    AudioDeviceId, AudioDeviceInfo, AudioDeviceKind, AudioDeviceList, CaptureError, CapturePhase,
    DeviceSelection,
};
use gijirec_presentation::tauri::device_selection::{
    RecordingDeviceSelectionEventEmitter, set_audio_device_ui_visible_impl,
    set_device_selection_impl, set_device_selection_with_capture_feedback,
};
use gijirec_presentation::tauri::events::RecordingEventEmitter;
use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
        inputs: vec![
            mic("mic-default", true),
            mic("mic-usb", false),
            mic("mic-unavailable", false),
        ],
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

struct SelectiveMicPort {
    opened: Arc<AtomicBool>,
    unavailable_ids: HashSet<String>,
    last_error: Arc<Mutex<Option<CaptureError>>>,
}

impl MicCapturePort for SelectiveMicPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        self.open_with_selection(None)
    }

    fn open_with_selection(
        &mut self,
        device_id: Option<&AudioDeviceId>,
    ) -> Result<(), CaptureError> {
        if let Some(id) = device_id
            && self.unavailable_ids.contains(id.as_str())
        {
            let err = CaptureError::SelectedMicUnavailable;
            *self.last_error.lock().expect("lock") = Some(err.clone());
            return Err(err);
        }
        self.opened.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn close(&mut self) {
        self.opened.store(false, Ordering::SeqCst);
    }

    fn is_open(&self) -> bool {
        self.opened.load(Ordering::SeqCst)
    }
}

struct SimpleSystemPort {
    opened: Arc<AtomicBool>,
}

impl SimpleSystemPort {
    fn new() -> Self {
        Self {
            opened: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SystemAudioCapturePort for SimpleSystemPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        self.open_with_selection(None)
    }

    fn open_with_selection(
        &mut self,
        _device_id: Option<&AudioDeviceId>,
    ) -> Result<(), CaptureError> {
        self.opened.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn close(&mut self) {
        self.opened.store(false, Ordering::SeqCst);
    }

    fn is_open(&self) -> bool {
        self.opened.load(Ordering::SeqCst)
    }
}

struct CaptureSelectionAdapter {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
}

impl CaptureSelectionPort for CaptureSelectionAdapter {
    fn capture_phase(&self) -> CapturePhase {
        self.orchestrator.lock().expect("lock").phase()
    }

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.orchestrator
            .lock()
            .expect("lock")
            .restart_with_selection(selection)
    }
}

struct IntegrationStack {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    device_selection: Arc<dyn DeviceSelectionService>,
    last_mic_error: Arc<Mutex<Option<CaptureError>>>,
}

impl IntegrationStack {
    fn new(unavailable_mic_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let last_mic_error = Arc::new(Mutex::new(None));
        let mic = SelectiveMicPort {
            opened: Arc::new(AtomicBool::new(false)),
            unavailable_ids: unavailable_mic_ids.into_iter().map(Into::into).collect(),
            last_error: Arc::clone(&last_mic_error),
        };
        let system = SimpleSystemPort::new();
        let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> =
            Arc::new(Mutex::new(DefaultCaptureOrchestrator::new(mic, system)));
        let device_selection: Arc<dyn DeviceSelectionService> =
            Arc::new(DefaultDeviceSelectionService::new(
                gijirec_presentation::application::device_selection::DeviceSelectionStore::new(),
                MockEnumerator {
                    list: sample_device_list(),
                },
                CaptureSelectionAdapter {
                    orchestrator: Arc::clone(&orchestrator),
                },
                NoopSpeakerPreflight,
                NoopEvents,
                SystemClock,
                Arc::new(NoopDeviceSelectionObservability),
            ));
        Self {
            orchestrator,
            device_selection,
            last_mic_error,
        }
    }

    fn start_capturing(&self) {
        let mut orch = self.orchestrator.lock().expect("lock");
        orch.start_with_selection(&DeviceSelection::default())
            .expect("initial start");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
    }
}

#[derive(Clone)]
struct MockClock {
    now: Arc<AtomicU64>,
}

impl MockClock {
    fn new(initial_ms: u64) -> Self {
        Self {
            now: Arc::new(AtomicU64::new(initial_ms)),
        }
    }

    fn advance(&self, ms: u64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }
}

impl DeviceSelectionClock for MockClock {
    fn now_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}

struct MutableMockEnumerator {
    list: Arc<Mutex<AudioDeviceList>>,
}

impl DeviceEnumeratorPort for MutableMockEnumerator {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
        Ok(self.list.lock().expect("lock").clone())
    }
}

struct HotplugIntegrationStack {
    service: Arc<
        DefaultDeviceSelectionService<
            MutableMockEnumerator,
            CaptureSelectionAdapter,
            NoopSpeakerPreflight,
            RecordingDeviceSelectionEventEmitter,
            MockClock,
        >,
    >,
    events: RecordingDeviceSelectionEventEmitter,
    list: Arc<Mutex<AudioDeviceList>>,
    clock: MockClock,
}

impl HotplugIntegrationStack {
    fn new(initial_list: AudioDeviceList, clock: MockClock) -> Self {
        let list = Arc::new(Mutex::new(initial_list));
        let last_mic_error = Arc::new(Mutex::new(None));
        let mic = SelectiveMicPort {
            opened: Arc::new(AtomicBool::new(false)),
            unavailable_ids: HashSet::new(),
            last_error: Arc::clone(&last_mic_error),
        };
        let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> = Arc::new(Mutex::new(
            DefaultCaptureOrchestrator::new(mic, SimpleSystemPort::new()),
        ));
        let events = RecordingDeviceSelectionEventEmitter::new();
        let service = Arc::new(DefaultDeviceSelectionService::new(
            gijirec_presentation::application::device_selection::DeviceSelectionStore::new(),
            MutableMockEnumerator {
                list: Arc::clone(&list),
            },
            CaptureSelectionAdapter {
                orchestrator: Arc::clone(&orchestrator),
            },
            NoopSpeakerPreflight,
            events.clone(),
            clock.clone(),
            Arc::new(NoopDeviceSelectionObservability),
        ));
        Self {
            service,
            events,
            list,
            clock,
        }
    }

    fn poll_tick(&self) {
        self.service
            .poll_tick_for_test()
            .expect("hotplug poll tick");
    }
}

/// Integration Test 1: `set_device_selection` → phase `capturing` recovery (req 3.3).
#[test]
fn integration_set_device_selection_returns_to_capturing() {
    let stack = IntegrationStack::new(["mic-unavailable"]);
    stack.start_capturing();

    let emitter = RecordingEventEmitter::new();
    let new_selection = DeviceSelection::new(
        Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
        Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
    );

    let applied = set_device_selection_with_capture_feedback(
        stack.device_selection.as_ref(),
        &stack.orchestrator,
        &emitter,
        new_selection.clone(),
        || None,
    )
    .expect("selection change while capturing");

    assert_eq!(applied, new_selection);
    assert_eq!(
        stack.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Capturing
    );
    let phases = emitter.phases();
    assert!(
        phases.iter().any(|payload| payload.phase == "capturing"),
        "expected capturing phase event after successful selection change: {phases:?}"
    );
}

/// Integration Test 2: listed but unavailable mic → `SELECTED_MIC_UNAVAILABLE` emit (req 4.1).
#[test]
fn integration_unavailable_selected_mic_emits_selected_mic_unavailable() {
    let stack = IntegrationStack::new(["mic-unavailable"]);
    stack.start_capturing();

    let emitter = RecordingEventEmitter::new();
    let bad_selection = DeviceSelection::new(
        Some(AudioDeviceId::new("mic-unavailable".to_string()).expect("id")),
        Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
    );

    let err = set_device_selection_with_capture_feedback(
        stack.device_selection.as_ref(),
        &stack.orchestrator,
        &emitter,
        bad_selection,
        || stack.last_mic_error.lock().expect("lock").clone(),
    )
    .expect_err("restart must fail for unavailable mic");

    assert_eq!(err.code, "INTERNAL");
    assert_eq!(
        stack.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Error
    );

    let errors = emitter.errors();
    assert_eq!(
        errors.len(),
        1,
        "expected one capture error event: {errors:?}"
    );
    assert_eq!(errors[0].code, "SELECTED_MIC_UNAVAILABLE");
    assert!(!errors[0].action_ja.is_empty());

    let phases = emitter.phases();
    assert!(
        phases.iter().any(|payload| payload.phase == "error"),
        "expected error phase event: {phases:?}"
    );
}

/// Integration Test 3: `devices-changed` only while UI visible (req 1.4, 5.1).
#[test]
fn integration_devices_changed_only_when_ui_visible() {
    let stack = HotplugIntegrationStack::new(sample_device_list(), MockClock::new(1_000));

    stack.poll_tick();
    assert!(
        stack.events.devices().is_empty(),
        "poll while UI hidden must not emit devices-changed"
    );

    set_audio_device_ui_visible_impl(stack.service.as_ref(), true);
    let visible_events = stack.events.devices();
    assert_eq!(
        visible_events.len(),
        1,
        "set_ui_visible(true) must emit initial devices-changed: {visible_events:?}"
    );
    assert_eq!(visible_events[0].devices, sample_device_list());
    assert_eq!(visible_events[0].timestamp_ms, 1_000);

    stack.poll_tick();
    assert_eq!(
        stack.events.devices().len(),
        1,
        "poll within 2s interval must not emit again"
    );

    set_audio_device_ui_visible_impl(stack.service.as_ref(), false);

    stack.clock.advance(HOTPLUG_POLL_INTERVAL_MS);
    stack
        .list
        .lock()
        .expect("lock")
        .inputs
        .push(mic("mic-hotplug", false));

    stack.poll_tick();
    assert_eq!(
        stack.events.devices().len(),
        1,
        "poll after UI hidden must not emit even when device list changes"
    );
}

struct RecordingChunkConsumer {
    chunks: Mutex<Vec<PcmChunk>>,
}

impl RecordingChunkConsumer {
    fn new() -> Self {
        Self {
            chunks: Mutex::new(Vec::new()),
        }
    }

    fn chunks(&self) -> Vec<PcmChunk> {
        self.chunks.lock().expect("lock").clone()
    }

    fn sequences(&self) -> Vec<u64> {
        self.chunks
            .lock()
            .expect("lock")
            .iter()
            .map(PcmChunk::sequence)
            .collect()
    }
}

impl PcmChunkConsumer for RecordingChunkConsumer {
    fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError> {
        self.chunks.lock().expect("lock").push(chunk);
        Ok(())
    }
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
        stop_processing_for_recapture(&self.pipeline);
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

struct PcmSequenceStack {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    device_selection: Arc<dyn DeviceSelectionService>,
    pipeline: Arc<CapturePipelineState>,
    chunks: Arc<RecordingChunkConsumer>,
    mic_open_count: Arc<AtomicUsize>,
    mic_close_count: Arc<AtomicUsize>,
    sys_open_count: Arc<AtomicUsize>,
    sys_close_count: Arc<AtomicUsize>,
    mic_prod: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
    sys_prod: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
}

impl PcmSequenceStack {
    fn new() -> Self {
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
        let chunks = Arc::new(RecordingChunkConsumer::new());
        pipeline
            .pcm_bus
            .register(Arc::clone(&chunks) as Arc<dyn PcmChunkConsumer>);

        let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> = Arc::new(Mutex::new(
            DefaultCaptureOrchestrator::new(mic_port, sys_port),
        ));
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
                Arc::new(NoopDeviceSelectionObservability),
            ));

        Self {
            orchestrator,
            device_selection,
            pipeline,
            chunks,
            mic_open_count,
            mic_close_count,
            sys_open_count,
            sys_close_count,
            mic_prod,
            sys_prod,
        }
    }

    fn start_capturing(&self) {
        let mut orch = self.orchestrator.lock().expect("lock");
        orch.start_with_selection(&DeviceSelection::default())
            .expect("initial start");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        drop(orch);
        self.pipeline.on_capture_started();
        assert!(processing_is_active(&self.pipeline));
    }

    fn pump_samples(&self, samples: usize) {
        let mut mic = self.mic_prod.lock().expect("lock");
        let mut sys = self.sys_prod.lock().expect("lock");
        let mic = mic.as_mut().expect("mic producer must be open");
        let sys = sys.as_mut().expect("sys producer must be open");
        for i in 0..samples {
            let sample = 0.2 * ((i as f32) * 0.01).sin();
            let _ = mic.push(sample);
            let _ = sys.push(sample * 0.5);
        }
    }
}

fn wait_for_chunk_count(recorder: &RecordingChunkConsumer, count: usize, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while recorder.chunks().len() < count && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
}

fn assert_strictly_increasing_sequences(sequences: &[u64]) {
    assert!(
        !sequences.is_empty(),
        "expected at least one PcmChunk sequence"
    );
    for window in sequences.windows(2) {
        assert_eq!(
            window[1],
            window[0] + 1,
            "PcmChunk.sequence must increase by 1 without gaps or reset: {sequences:?}"
        );
    }
}

/// Integration Test 4: selection change → `PcmChunk.sequence` monotonic continuity (req 3.2).
#[test]
#[allow(clippy::too_many_lines)] // Integration test: full capture + selection-change sequence assertions.
fn integration_pcm_sequence_continues_after_selection_change() {
    let stack = PcmSequenceStack::new();
    stack.start_capturing();

    let frames_per_chunk = CHUNK_FRAME_COUNT as usize;
    stack.pump_samples(frames_per_chunk * 4);
    wait_for_chunk_count(&stack.chunks, 3, Duration::from_secs(2));

    let before_change = stack.chunks.sequences();
    assert!(
        before_change.len() >= 3,
        "expected at least 3 chunks before selection change: {before_change:?}"
    );
    assert_strictly_increasing_sequences(&before_change);
    let last_before = *before_change.last().expect("last seq");
    let emitter_seq_before = stack
        .pipeline
        .chunk_emitter
        .lock()
        .expect("lock")
        .next_sequence();
    assert!(emitter_seq_before >= 3);

    let mic_open_before = stack.mic_open_count.load(Ordering::SeqCst);
    let mic_close_before = stack.mic_close_count.load(Ordering::SeqCst);
    assert_eq!(mic_open_before, 1, "initial capture must open mic once");

    let new_selection = DeviceSelection::new(
        Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
        Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
    );
    let applied = set_device_selection_impl(stack.device_selection.as_ref(), new_selection.clone())
        .expect("selection change while capturing");
    assert_eq!(applied, new_selection);
    assert_eq!(
        stack.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Capturing
    );
    assert!(
        processing_is_active(&stack.pipeline),
        "processing must restart after recapture"
    );
    assert!(
        stack.mic_close_count.load(Ordering::SeqCst) > mic_close_before,
        "recapture must close mic port"
    );
    assert!(
        stack.mic_open_count.load(Ordering::SeqCst) > mic_open_before,
        "recapture must reopen mic port"
    );
    assert!(
        stack.sys_close_count.load(Ordering::SeqCst) >= 1,
        "recapture must close system port"
    );
    assert!(
        stack.sys_open_count.load(Ordering::SeqCst) >= 2,
        "recapture must reopen system port"
    );

    stack.pump_samples(frames_per_chunk * 4);
    wait_for_chunk_count(
        &stack.chunks,
        before_change.len() + 3,
        Duration::from_secs(2),
    );

    let all_sequences = stack.chunks.sequences();
    assert_strictly_increasing_sequences(&all_sequences);

    let after_change: Vec<u64> = all_sequences
        .iter()
        .copied()
        .skip(before_change.len())
        .collect();
    assert!(
        !after_change.is_empty(),
        "expected chunks after selection change"
    );
    assert_eq!(
        after_change[0],
        last_before + 1,
        "sequence must not reset on recapture: before_last={last_before} after_first={}",
        after_change[0]
    );
    assert_eq!(
        stack
            .pipeline
            .chunk_emitter
            .lock()
            .expect("lock")
            .next_sequence(),
        after_change.last().expect("last after") + 1,
        "shared ChunkEmitter must continue after processing restart"
    );

    stack.pipeline.on_capture_stopping();
}
