use super::*;
use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
use std::sync::atomic::{AtomicU8, Ordering};

fn compose_production_source() -> String {
    let compose_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compose");
    let mut files: Vec<_> = std::fs::read_dir(&compose_dir)
        .expect("compose dir must exist")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            path.extension().is_some_and(|ext| ext == "rs")
                && path.file_name().is_some_and(|name| name != "tests.rs")
        })
        .collect();
    files.sort_by_key(|entry| entry.path());
    files
        .into_iter()
        .map(|entry| std::fs::read_to_string(entry.path()).expect("read compose module file"))
        .collect::<Vec<_>>()
        .join("\n")
}

struct PortOpenOrder(Arc<AtomicU8>);

impl PortOpenOrder {
    fn close(&self) {
        self.0.store(0, Ordering::SeqCst);
    }

    fn is_at_least(&self, min: u8) -> bool {
        self.0.load(Ordering::SeqCst) >= min
    }

    fn is_exactly(&self, v: u8) -> bool {
        self.0.load(Ordering::SeqCst) == v
    }
}

macro_rules! tracking_capture_port_lifecycle {
    ($method:ident, $arg:expr) => {
        fn close(&mut self) {
            self.order.close();
        }

        fn is_open(&self) -> bool {
            self.order.$method($arg)
        }
    };
}

struct TrackingMic {
    order: PortOpenOrder,
}

struct TrackingSystem {
    order: PortOpenOrder,
}

impl gijirec_presentation::application::capture::orchestrator::MicCapturePort for TrackingMic {
    fn open(&mut self) -> Result<(), CaptureError> {
        self.order.0.store(1, Ordering::SeqCst);
        Ok(())
    }

    tracking_capture_port_lifecycle!(is_at_least, 1);
}

impl gijirec_presentation::application::capture::orchestrator::SystemAudioCapturePort
    for TrackingSystem
{
    fn open(&mut self) -> Result<(), CaptureError> {
        let mic_first = self.order.0.load(Ordering::SeqCst) == 1;
        if !mic_first {
            return Err(CaptureError::Internal {
                detail: "system opened before mic".to_string(),
            });
        }
        self.order.0.store(2, Ordering::SeqCst);
        Ok(())
    }

    tracking_capture_port_lifecycle!(is_exactly, 2);
}

#[test]
fn inject_model_stack_accepts_app_data_dir() {
    let composed = build_capture_stack();
    inject_model_stack_shared(
        &composed.model_orchestrator,
        std::env::temp_dir().join("gijirec_inject_test"),
    );
}

#[test]
fn compose_does_not_reference_dirs_data_local_dir() {
    let production_source = compose_production_source();
    assert!(
        !production_source.contains("dirs::data_local_dir"),
        "compose production code must not use dirs::data_local_dir; app_data_dir is injected after Tauri setup"
    );
}

#[test]
fn compose_holds_pipeline_components() {
    let (streams, _mic, _system) = CaptureStreamHandles::new_pair();
    let order = PortOpenOrder(Arc::new(AtomicU8::new(0)));
    let mic = TrackingMic {
        order: PortOpenOrder(Arc::clone(&order.0)),
    };
    let system = TrackingSystem { order };
    let composed = compose_with_ports(mic, system, streams);

    assert_eq!(
        composed.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Idle
    );
    assert!(!composed.pipeline.processing_is_active());
}

#[test]
fn device_selection_service_is_wired_in_composed_stack() {
    let composed = build_capture_stack();
    let _ = composed
        .device_selection
        .list_devices()
        .expect("list_devices must not panic");
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn pcm_rtrb_capacity_exceeds_inference_window_with_backlog_headroom() {
    use wiring::PCM_INFERENCE_WINDOW_SAMPLES;

    assert!(
        PCM_RTRB_CAPACITY_SAMPLES > PCM_INFERENCE_WINDOW_SAMPLES,
        "rtrb must exceed one 30 s window so ingest survives slow inference"
    );
    assert!(
        PCM_RTRB_CAPACITY_SAMPLES >= 2 * PCM_INFERENCE_WINDOW_SAMPLES,
        "rtrb should hold at least two windows for inference overlap plus capture"
    );
    let (_prod, _cons) = rtrb::RingBuffer::<f32>::new(PCM_RTRB_CAPACITY_SAMPLES);
}

#[test]
fn build_capture_stack_wires_stall_watchdog() {
    let composed = build_capture_stack();
    assert!(
        composed.transcribe_lifecycle.stall_watchdog().is_some(),
        "production compose must enable stall watchdog via with_stall_watchdog"
    );
}

#[test]
fn compose_wires_worker_fatal_and_engine_ready_to_lifecycle() {
    let production_source = compose_production_source();
    assert!(
        production_source.contains("on_worker_engine_failed"),
        "compose must surface worker engine load failures through the lifecycle hook"
    );
    assert!(
        production_source.contains("on_engine_ready"),
        "compose must reset stall timing after worker engine load completes"
    );
    assert!(
        production_source.contains("on_inference_attempted"),
        "compose must reset stall timing when inference starts"
    );
    assert!(
        production_source.contains("on_inference_progress"),
        "compose must extend stall timing from whisper.cpp progress callbacks"
    );
}

#[test]
fn compose_wires_batch_observability_and_shared_rtrb_overflow_counter() {
    let production_source = compose_production_source();
    assert!(
        production_source.contains("set_batch_cycle_started_callback"),
        "compose must wire batch cycle started observability"
    );
    assert!(
        production_source.contains("set_batch_cycle_completed_callback"),
        "compose must wire batch cycle completed observability"
    );
    assert!(
        production_source.contains("set_rtrb_overflow_counter"),
        "compose must share rtrb overflow counter between ingest and worker"
    );
    assert!(
        production_source.contains("log_batch_cycle_started"),
        "compose must dispatch batch cycle started tracing"
    );
}

struct ComposeBatchMockEngine {
    segments: Vec<gijirec_presentation::infrastructure::transcribe::WhisperSegment>,
}

impl gijirec_presentation::infrastructure::transcribe::SegmentEngine for ComposeBatchMockEngine {
    fn transcribe_pcm(
        &mut self,
        _pcm: &[f32],
    ) -> Result<
        Vec<gijirec_presentation::infrastructure::transcribe::WhisperSegment>,
        gijirec_presentation::domain::transcribe::TranscribeError,
    > {
        Ok(self.segments.clone())
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

impl gijirec_presentation::infrastructure::transcribe::ModelPathLoadable
    for ComposeBatchMockEngine
{
    fn load_from_path_if_needed(
        &mut self,
        _path: &std::path::Path,
    ) -> Result<(), gijirec_presentation::domain::transcribe::TranscribeError> {
        Ok(())
    }
}

/// Mirrors compose wiring: pcm_bus → ingest → expanded rtrb → batch worker → mock adapter → blocks.
#[test]
#[allow(clippy::assertions_on_constants)]
fn compose_batch_pipeline_end_to_end_synthetic_pcm_to_blocks() {
    use gijirec_presentation::application::transcribe::block_emitter::BlockEmitter;
    use gijirec_presentation::domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
    use gijirec_presentation::domain::transcribe::TranscriptSegmentSink;
    use gijirec_presentation::infrastructure::transcribe::TranscribeWorker;
    use gijirec_presentation::tauri::pcm_bus::MAX_QUEUED_CHUNKS;
    use gijirec_presentation::transcribe::PcmIngestConsumer;
    use std::thread;
    use std::time::Duration;
    use wiring::PCM_INFERENCE_WINDOW_SAMPLES;

    assert!(
        PCM_RTRB_CAPACITY_SAMPLES >= PCM_INFERENCE_WINDOW_SAMPLES * 2,
        "compose rtrb must exceed batch window for ingest headroom"
    );
    assert_eq!(MAX_QUEUED_CHUNKS, 3000);

    let (pcm_prod, pcm_cons) = rtrb::RingBuffer::<f32>::new(PCM_RTRB_CAPACITY_SAMPLES);
    let pcm_ingest = PcmIngestConsumer::new(pcm_prod);
    let rtrb_overflow_counter = pcm_ingest.rtrb_overflow_counter();
    let pcm_ingest = Arc::new(pcm_ingest);
    let pcm_bus = Arc::new(gijirec_presentation::tauri::pcm_bus::PcmChunkBus::new());
    pcm_bus.register(pcm_ingest);

    let (block_bus, recorded_blocks) =
        gijirec_presentation::transcribe::test_support::recording_block_bus();

    let emitter = Arc::new(BlockEmitter::new(Arc::clone(&block_bus)));
    let engine = ComposeBatchMockEngine {
        segments: vec![
            gijirec_presentation::infrastructure::transcribe::WhisperSegment {
                text: "batch compose".to_string(),
                start_ms: 500,
                end_ms: 1500,
            },
        ],
    };
    let mut worker = TranscribeWorker::with_engine(
        Arc::clone(&emitter) as Arc<dyn TranscriptSegmentSink>,
        engine,
    );
    worker.attach_pcm_consumer(pcm_cons);
    worker.set_rtrb_overflow_counter(rtrb_overflow_counter);
    worker.spawn().expect("spawn batch worker");

    let chunks_for_30s = PCM_INFERENCE_WINDOW_SAMPLES / CHUNK_FRAME_COUNT as usize;
    for seq in 0..chunks_for_30s {
        let samples = vec![16384_i16; CHUNK_FRAME_COUNT as usize];
        let chunk = PcmChunk::new(seq as u64, samples, seq as u64 * 100).expect("chunk");
        pcm_bus.publish(chunk);
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while recorded_blocks.lock().expect("lock").is_empty() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }

    worker
        .stop_and_join(Duration::from_secs(2))
        .expect("stop batch worker");

    let blocks = recorded_blocks.lock().expect("lock");
    assert_eq!(
        blocks.len(),
        1,
        "mock adapter must emit one block through compose wiring"
    );
    assert_eq!(blocks[0].text, "batch compose");
    assert_eq!(blocks[0].sequence, 1);
    assert_eq!(blocks[0].start_timestamp_ms, 500);
}

/// Hardware integration test for Windows WASAPI loopback + mic (ignored on CI).
#[test]
#[ignore = "CI: requires Windows mic permission and default WASAPI loopback output; run with --ignored on local hardware"]
fn integration_mic_and_wasapi_loopback_reach_capturing_on_hardware() {
    let composed = build_capture_stack();
    assert_eq!(
        composed.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Idle
    );
}

/// Hardware integration test for macOS ScreenCaptureKit + mic (ignored on CI).
#[test]
#[ignore = "CI: requires macOS mic permission and ScreenCaptureKit screen recording permission; run with --ignored on local hardware"]
fn integration_mic_and_sck_reach_capturing_on_hardware() {
    let composed = build_capture_stack();
    assert_eq!(
        composed.orchestrator.lock().expect("lock").phase(),
        CapturePhase::Idle
    );
}

#[test]
fn compose_wires_capture_audio_controls_stack() {
    let production_source = compose_production_source();
    for needle in [
        "CaptureAudioControlsApplyPortAdapter",
        "ComposeIngestSourcePort",
        "CaptureAudioControlsProcessingHook",
        "ingest_level_emitter.pcm_rms_callback",
        "LateBoundCaptureAudioControlsEvents",
    ] {
        assert!(
            production_source.contains(needle),
            "compose production code must wire capture audio controls via {needle}"
        );
    }
}

#[test]
fn composed_stack_exposes_capture_audio_controls_service() {
    let composed = build_capture_stack();
    let state = composed.capture_audio_controls.get_state();
    assert!(state.controls.mic_ingest_enabled);
    assert!(composed.ingest_level_cache.lock().expect("lock").is_none());
}

#[test]
fn capture_audio_controls_hook_applies_stored_gain_on_capture_start() {
    use gijirec_presentation::application::capture_audio_controls::CaptureAudioControlsPatch;
    use gijirec_presentation::domain::audio::DEFAULT_INGEST_GAIN;

    let composed = build_capture_stack();
    composed
        .capture_audio_controls
        .apply_partial(CaptureAudioControlsPatch {
            manual_ingest_gain: Some(2.5),
            mic_ingest_enabled: Some(false),
            ..Default::default()
        })
        .expect("apply");

    assert!(
        (composed
            .capture_audio_controls_hook
            .pcm_ingest()
            .ingest_gain_multiplier()
            - DEFAULT_INGEST_GAIN)
            .abs()
            < f32::EPSILON,
        "gain must not apply until capturing"
    );

    composed.capture_audio_controls_hook.on_capture_started();

    assert!(!composed.pipeline.mic_gate().mic_ingest_enabled());
    assert!(
        (composed
            .capture_audio_controls_hook
            .pcm_ingest()
            .ingest_gain_multiplier()
            - 2.5)
            .abs()
            < f32::EPSILON
    );
}

#[test]
fn pcm_ingest_rms_accumulator_emits_every_fifty_chunks() {
    use wiring::PcmIngestRmsAccumulator;

    let mut acc = PcmIngestRmsAccumulator::new();
    for i in 1..50 {
        assert!(acc.observe(i as f32 * 0.001).is_none());
    }
    let (min, max, mean, count) = acc.observe(0.05).expect("summary");
    assert_eq!(count, 50);
    assert!((min - 0.001).abs() < f32::EPSILON);
    assert!((max - 0.05).abs() < f32::EPSILON);
    assert!(mean > 0.0);
}
