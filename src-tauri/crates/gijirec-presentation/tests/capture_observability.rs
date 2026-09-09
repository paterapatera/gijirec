use gijirec_presentation::domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
use gijirec_presentation::tauri::observability::{
    RecordingObservability, log_phase_transition, log_stream_open_failure, set_observability,
};
use gijirec_presentation::tauri::pcm_bus::{MAX_QUEUED_CHUNKS, PcmChunkBus};
use std::sync::Mutex;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn looks_like_pcm_dump(text: &str) -> bool {
    text.contains("[12345,")
        || text.contains(", 12345,")
        || (text.matches(',').count() > 50 && text.contains("12345"))
}

#[test]
fn phase_transition_records_capture_phase_without_pcm() {
    let _guard = TEST_LOCK.lock().expect("lock");
    let recorder = RecordingObservability::new();
    set_observability(Box::new(recorder.clone()));

    log_phase_transition(CapturePhase::Starting);
    log_phase_transition(CapturePhase::Capturing);

    let phases = recorder.phases.lock().expect("lock");
    assert_eq!(
        phases.as_slice(),
        &[CapturePhase::Starting, CapturePhase::Capturing]
    );
    let debug = format!("{phases:?}");
    assert!(!looks_like_pcm_dump(&debug));
}

#[test]
fn buffer_drop_records_capture_buffer_drops_total_without_pcm() {
    let _guard = TEST_LOCK.lock().expect("lock");
    let recorder = RecordingObservability::new();
    set_observability(Box::new(recorder.clone()));

    let bus = PcmChunkBus::new();
    let overflow = 2usize;
    for seq in 0..(MAX_QUEUED_CHUNKS as u64 + overflow as u64) {
        let chunk =
            PcmChunk::new(seq, vec![0_i16; CHUNK_FRAME_COUNT as usize], seq * 100).expect("chunk");
        bus.publish(chunk);
    }

    assert_eq!(bus.buffer_drops_total(), overflow as u64);
    let drops = recorder.drops.lock().expect("lock");
    assert_eq!(drops.as_slice(), &[1, 2]);
    let debug = format!("{drops:?}");
    assert!(!looks_like_pcm_dump(&debug));
}

#[test]
fn stream_failure_records_error_code_not_device_name() {
    let _guard = TEST_LOCK.lock().expect("lock");
    let recorder = RecordingObservability::new();
    set_observability(Box::new(recorder.clone()));

    log_stream_open_failure("mic", &CaptureError::MicUnavailable, "sess-test");

    let failures = recorder.stream_failures.lock().expect("lock");
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, "mic");
    assert_eq!(failures[0].1, CaptureError::MicUnavailable);
    let debug = format!("{failures:?}");
    assert!(!debug.contains("Microphone ("));
    assert!(!debug.contains("default_input"));
}
