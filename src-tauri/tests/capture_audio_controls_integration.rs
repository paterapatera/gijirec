//! Integration tests for capture-audio-controls (task 10.1).

use gijirec_lib::capture_audio_controls_integration_support::CaptureAudioControlsIntegrationStack;
use gijirec_presentation::domain::audio::pcm_chunk::PcmChunkConsumer;
use gijirec_presentation::domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
use gijirec_presentation::domain::audio::{CaptureError, DEFAULT_INGEST_GAIN};
use gijirec_presentation::tauri::capture_audio_controls::CaptureAudioControlsPatchRequest;
use gijirec_presentation::transcribe::PcmIngestConsumer;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const PCM_SAMPLE_RATE_HZ: u32 = 16_000;
const PCM_INFERENCE_WINDOW_SAMPLES: usize = 30 * PCM_SAMPLE_RATE_HZ as usize;
const PCM_RTRB_CAPACITY_SAMPLES: usize = PCM_INFERENCE_WINDOW_SAMPLES * 10;

fn make_chunk(seq: u64, amplitude: i16) -> PcmChunk {
    PcmChunk::new(seq, vec![amplitude; CHUNK_FRAME_COUNT as usize], seq * 100).expect("chunk")
}

fn drain_pcm_buffer(cons: &mut rtrb::Consumer<f32>) {
    while cons.pop().is_ok() {}
}

/// set_capture_audio_controls → PcmIngestConsumer gain reflection while capturing.
#[test]
fn integration_set_capture_audio_controls_reflects_gain_on_pcm_ingest() {
    let stack = CaptureAudioControlsIntegrationStack::new();
    stack.start_capturing();

    assert!(
        (stack.pcm_ingest().ingest_gain_multiplier() - DEFAULT_INGEST_GAIN).abs() < f32::EPSILON,
        "capture start must apply default gain"
    );

    let response = stack.set_controls(CaptureAudioControlsPatchRequest {
        manual_ingest_gain: Some(2.5),
        ..Default::default()
    });
    assert_eq!(response.controls.manual_ingest_gain, 2.5);
    assert!((stack.pcm_ingest().ingest_gain_multiplier() - 2.5).abs() < f32::EPSILON);

    stack.on_capture_stopping();
}

/// mic OFF + no system processing → TRANSCRIBE_INGEST_NO_AUDIO_SOURCE.
#[test]
fn integration_mic_off_without_system_emits_transcribe_ingest_no_audio_source() {
    let stack = CaptureAudioControlsIntegrationStack::new();
    stack.start_capturing();
    stack.stop_processing_keep_capturing();

    stack.set_controls(CaptureAudioControlsPatchRequest {
        mic_ingest_enabled: Some(false),
        ..Default::default()
    });

    let errors = stack.events().capture_errors();
    assert_eq!(errors.len(), 1, "expected one capture error: {errors:?}");
    assert_eq!(errors[0], CaptureError::TranscribeIngestNoAudioSource);

    stack.on_capture_stopping();
}

/// Device recapture simulation preserves session controls in the store.
#[test]
fn integration_controls_preserved_after_device_recapture() {
    let stack = CaptureAudioControlsIntegrationStack::new();
    stack.start_capturing();

    let expected = stack
        .set_controls(CaptureAudioControlsPatchRequest {
            mic_ingest_enabled: Some(false),
            manual_ingest_gain: Some(3.75),
            ..Default::default()
        })
        .controls;

    let new_selection = stack.alternate_device_selection();
    stack.simulate_device_recapture(new_selection);
    assert!(
        stack.processing_is_active(),
        "recapture must restart processing"
    );

    let after = stack.get_controls().controls;
    assert_eq!(after.mic_ingest_enabled, expected.mic_ingest_enabled);
    assert_eq!(after.manual_ingest_gain, expected.manual_ingest_gain);
    assert_eq!(after.gain_user_adjusted, expected.gain_user_adjusted);

    stack.on_capture_stopping();
}

/// Non-capturing store update applies to ingest on the next capture start.
#[test]
fn integration_non_capturing_store_applies_on_next_capture_start() {
    let stack = CaptureAudioControlsIntegrationStack::new();

    stack.set_controls(CaptureAudioControlsPatchRequest {
        manual_ingest_gain: Some(3.0),
        mic_ingest_enabled: Some(false),
        ..Default::default()
    });

    assert!(
        (stack.pcm_ingest().ingest_gain_multiplier() - DEFAULT_INGEST_GAIN).abs() < f32::EPSILON,
        "gain must not apply live while idle"
    );

    stack.start_capturing();

    assert!(!stack.mic_ingest_enabled_in_store());
    assert!((stack.pcm_ingest().ingest_gain_multiplier() - 3.0).abs() < f32::EPSILON);

    stack.on_capture_stopping();
}

struct NoopIngestLevelEventEmitter;

impl gijirec_presentation::transcribe::IngestLevelEventEmitter for NoopIngestLevelEventEmitter {
    fn emit_ingest_level(
        &self,
        _payload: gijirec_presentation::transcribe::IngestLevelChangedPayload,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Ingest-level 1 Hz metering must not increase rtrb overflow when the drain keeps up.
#[test]
fn integration_ingest_level_1hz_does_not_increase_rtrb_overflow() {
    use gijirec_presentation::tauri::pcm_bus::PcmChunkBus;
    use gijirec_presentation::transcribe::IngestLevelEmitter;

    let (pcm_prod, mut pcm_cons) = rtrb::RingBuffer::<f32>::new(PCM_RTRB_CAPACITY_SAMPLES);
    let mut pcm_ingest = PcmIngestConsumer::new(pcm_prod);
    let overflow_before = pcm_ingest.rtrb_overflow_count();

    let ingest_level_events = Arc::new(NoopIngestLevelEventEmitter);
    let ingest_level_emitter = IngestLevelEmitter::new(ingest_level_events);
    ingest_level_emitter.set_supplying(true);
    let ingest_level_rms = ingest_level_emitter.pcm_rms_callback();
    pcm_ingest.set_pcm_rms_callback(ingest_level_rms);

    let pcm_ingest = Arc::new(pcm_ingest);
    let pcm_bus = Arc::new(PcmChunkBus::new());
    pcm_bus.register(Arc::clone(&pcm_ingest) as Arc<dyn PcmChunkConsumer>);

    let draining = Arc::new(AtomicBool::new(true));
    let drain_handle = thread::spawn({
        let draining = Arc::clone(&draining);
        move || {
            while draining.load(Ordering::Relaxed) {
                drain_pcm_buffer(&mut pcm_cons);
                thread::sleep(Duration::from_millis(1));
            }
            drain_pcm_buffer(&mut pcm_cons);
        }
    });

    // ~3 s of chunks at 10 Hz plus ingest-level aggregation ticks.
    for seq in 0..30 {
        pcm_bus.publish(make_chunk(seq as u64, 12_000));
        ingest_level_emitter.tick_at(std::time::Instant::now());
        thread::sleep(Duration::from_millis(100));
    }

    draining.store(false, Ordering::Relaxed);
    drain_handle.join().expect("drain thread");

    let overflow_after = pcm_ingest.rtrb_overflow_count();
    assert_eq!(
        overflow_after, overflow_before,
        "1 Hz ingest-level path must not add rtrb overflow beyond drain baseline"
    );
}
