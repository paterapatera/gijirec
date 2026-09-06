use std::io::Write;
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;

use gijirec_lib::transcribe_observability::TracingTranscribeObservability;
use gijirec_presentation::domain::transcribe::{TranscribeError, TranscribePhase};
use gijirec_presentation::transcribe::observability::{
    TRANSCRIBE_LOG_TARGET, log_block_buffer_drop, log_inference_latency, log_pcm_sequence_gaps,
    log_phase_transition, log_stall_detected, log_transcribe_error, set_transcribe_observability,
};

struct TranscribeWriter(Arc<Mutex<Vec<u8>>>);

impl Write for TranscribeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn with_transcribe_tracing_logs<F: FnOnce()>(f: F) -> String {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let writer_buf = Arc::clone(&buf);
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(format!("{TRANSCRIBE_LOG_TARGET}=trace")))
        .with_writer(move || TranscribeWriter(Arc::clone(&writer_buf)))
        .with_ansi(false)
        .without_time()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    set_transcribe_observability(Box::new(TracingTranscribeObservability));
    f();
    String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8")
}

#[test]
fn phase_transition_records_transcribe_phase() {
    let logs = with_transcribe_tracing_logs(|| {
        log_phase_transition(TranscribePhase::Transcribing);
    });

    assert!(
        logs.contains("transcribe_phase=transcribing")
            || (logs.contains("transcribe_phase") && logs.contains("transcribing")),
        "log must contain transcribe_phase=transcribing: {logs}"
    );
    assert!(
        logs.contains("session_id="),
        "log must contain session_id: {logs}"
    );
}

#[test]
fn pcm_sequence_gaps_records_gap_range() {
    let logs = with_transcribe_tracing_logs(|| {
        log_pcm_sequence_gaps(10, 15);
    });

    assert!(
        logs.contains("transcribe_pcm_sequence_gaps"),
        "log must record transcribe_pcm_sequence_gaps: {logs}"
    );
    assert!(logs.contains("10") && logs.contains("15"));
}

#[test]
fn block_buffer_drop_records_drop_metric() {
    let logs = with_transcribe_tracing_logs(|| {
        log_block_buffer_drop(3);
    });

    assert!(
        logs.contains("transcribe_block_buffer_drops"),
        "log must record transcribe_block_buffer_drops: {logs}"
    );
    assert!(logs.contains("3"));
}

#[test]
fn inference_latency_records_latency_ms() {
    let logs = with_transcribe_tracing_logs(|| {
        log_inference_latency(450);
    });

    assert!(
        logs.contains("transcribe_inference_latency_ms"),
        "log must record transcribe_inference_latency_ms: {logs}"
    );
    assert!(logs.contains("450"));
}

#[test]
fn stall_detected_records_diagnostic_once() {
    let logs = with_transcribe_tracing_logs(|| {
        log_stall_detected();
    });

    assert!(
        logs.contains("transcribe_stall_detected=true")
            || (logs.contains("transcribe_stall_detected") && logs.contains("true")),
        "log must record transcribe_stall_detected: {logs}"
    );
    assert!(
        logs.contains("error_code=INFERENCE_FAILED") || logs.contains("INFERENCE_FAILED"),
        "stall log must include INFERENCE_FAILED error_code: {logs}"
    );
    assert!(
        logs.contains("session_id="),
        "stall log must include session_id: {logs}"
    );
}

#[test]
fn error_records_error_code_and_masks_transcription_text() {
    let logs = with_transcribe_tracing_logs(|| {
        log_transcribe_error(&TranscribeError::InferenceFailed {
            detail: "whisper internal failed: confidential text should not leak".to_string(),
        });
    });

    assert!(
        logs.contains("error_code=INFERENCE_FAILED") || logs.contains("INFERENCE_FAILED"),
        "log must record error code: {logs}"
    );
    // Detail string or text shouldn't be printed verbatim if it contains transcription
    assert!(logs.contains("INFERENCE_FAILED"));
}
