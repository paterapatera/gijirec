use gijirec_lib::capture_observability::TracingCaptureObservability;
use gijirec_presentation::domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
use gijirec_presentation::tauri::observability::{CAPTURE_LOG_TARGET, set_observability};
use gijirec_presentation::tauri::pcm_bus::PcmChunkBus;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;

struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn with_tracing_logs<F: FnOnce()>(f: F) -> String {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let writer_buf = Arc::clone(&buf);
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(format!("{CAPTURE_LOG_TARGET}=trace")))
        .with_writer(move || CaptureWriter(Arc::clone(&writer_buf)))
        .with_ansi(false)
        .without_time()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    set_observability(Box::new(TracingCaptureObservability));
    f();
    String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8")
}

fn looks_like_pcm_dump(text: &str) -> bool {
    text.contains("[12345,")
        || text.contains(", 12345,")
        || (text.matches(',').count() > 50 && text.contains("12345"))
}

#[test]
fn tracing_backend_emits_structured_fields_without_pcm_samples() {
    let logs = with_tracing_logs(|| {
        gijirec_presentation::tauri::observability::log_phase_transition(CapturePhase::Capturing);

        gijirec_presentation::tauri::observability::log_buffer_drop(2);

        let bus = PcmChunkBus::new();
        for seq in 0..5 {
            let chunk = PcmChunk::new(seq, vec![12345_i16; CHUNK_FRAME_COUNT as usize], seq * 100)
                .expect("chunk");
            bus.publish(chunk);
        }

        gijirec_presentation::tauri::observability::log_stream_open_failure(
            "mic",
            &CaptureError::MicUnavailable,
            "sess-test",
        );
    });

    assert!(
        logs.contains("capture_phase") && logs.contains("capturing"),
        "{logs}"
    );
    assert!(logs.contains("capture_buffer_drops_total"), "{logs}");
    assert!(
        logs.contains("MIC_UNAVAILABLE") || logs.contains("error_code"),
        "{logs}"
    );
    assert!(
        !looks_like_pcm_dump(&logs),
        "logs must not contain PCM dumps:\n{logs}"
    );
}
