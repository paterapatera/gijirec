use gijirec_lib::device_selection_observability::{
    DEVICE_SELECTION_LOG_TARGET, TracingDeviceSelectionObservability,
};
use gijirec_presentation::application::device_selection::DeviceSelectionObservability;
use gijirec_presentation::domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
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
        .with_env_filter(EnvFilter::new(format!(
            "{DEVICE_SELECTION_LOG_TARGET}=debug"
        )))
        .with_writer(move || CaptureWriter(Arc::clone(&writer_buf)))
        .with_ansi(false)
        .without_time()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    f();
    String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8")
}

fn looks_like_pcm_dump(text: &str) -> bool {
    text.contains("[12345,")
        || text.contains(", 12345,")
        || (text.matches(',').count() > 50 && text.contains("12345"))
}

#[test]
fn tracing_backend_logs_ids_and_restart_duration_without_pcm_or_info_names() {
    let obs = TracingDeviceSelectionObservability;
    let logs = with_tracing_logs(|| {
        obs.log_device_names_debug(Some("Built-in Microphone"), Some("Built-in Output"));
        obs.log_selection_changed(Some("mic-id-usb"), Some("spk-id-default"));
        obs.log_recapture_started("corr-1", Some("mic-id-usb"), Some("spk-id-default"));
        obs.log_recapture_completed("corr-1", 42);

        let chunk =
            PcmChunk::new(0, vec![12345_i16; CHUNK_FRAME_COUNT as usize], 0).expect("chunk");
        let _ = format!("{chunk:?}");
    });

    assert!(
        logs.contains("device selection changed"),
        "missing selection changed: {logs}"
    );
    assert!(
        logs.contains("microphone_id=\"mic-id-usb\""),
        "INFO must log device id: {logs}"
    );
    assert!(
        logs.contains("speaker_id=\"spk-id-default\""),
        "INFO must log speaker id: {logs}"
    );
    assert!(
        logs.contains("device selection recapture started"),
        "missing recapture started: {logs}"
    );
    assert!(
        logs.contains("correlation_id=\"corr-1\""),
        "missing correlation_id: {logs}"
    );
    assert!(
        logs.contains("device selection recapture completed"),
        "missing recapture completed: {logs}"
    );
    assert!(
        logs.contains("device_selection_restart_duration_ms=42"),
        "missing duration metric: {logs}"
    );
    assert!(
        logs.contains("device selection display names"),
        "DEBUG names event expected: {logs}"
    );
    assert!(
        logs.contains("microphone_name=\"Built-in Microphone\""),
        "DEBUG must log display name: {logs}"
    );
    assert!(
        !logs.contains("microphone_name=mic-id-usb"),
        "display name field must not reuse id at INFO: {logs}"
    );
    assert!(
        !looks_like_pcm_dump(&logs),
        "PCM samples must not appear in logs:\n{logs}"
    );
}
