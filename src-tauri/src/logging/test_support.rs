//! Shared helpers for release logging integration tests.

use super::build_release_file_tracing_stack;
use crate::capture_observability::TracingCaptureObservability;
use crate::editor_observability::TracingEditorObservability;
use crate::transcribe_observability::TracingTranscribeObservability;
use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
use gijirec_presentation::domain::transcribe::{TranscribeError, TranscribePhase};
use gijirec_presentation::editor::observability::{
    EditorObservability, EditorSaveCompletion, EditorSaveLogFields, save_log_fields,
};
use gijirec_presentation::tauri::observability::{CaptureObservability, init_session_id};
use gijirec_presentation::transcribe::observability::TranscribeObservability;
use std::path::{Path, PathBuf};

pub(super) fn read_log_file(log_path: &Path) -> String {
    std::fs::read_to_string(log_path).unwrap_or_default()
}

pub(super) fn unique_temp_dir(scope: &str, label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gijirec-logging-{scope}-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

pub(super) fn record_release_tracing_session(
    app_data: &Path,
    run_session_id: &str,
    emit_events: impl FnOnce(),
) -> PathBuf {
    let stack = build_release_file_tracing_stack(app_data, run_session_id).expect("tracing stack");
    let log_path = stack.log_path().to_path_buf();
    let (_, guard) = stack.run_with_default(|| {
        init_session_id();
        emit_events();
    });
    drop(guard);
    log_path
}

pub(super) fn editor_save_completion_success() -> EditorSaveCompletion {
    EditorSaveCompletion {
        success: true,
        files_written_count: 1,
        error_code: None,
    }
}

pub(super) fn log_editor_save_lifecycle(
    editor: &TracingEditorObservability,
    fields: &EditorSaveLogFields,
) {
    let completion = editor_save_completion_success();
    editor.log_save_started(fields);
    editor.log_save_completed(fields, &completion);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn smoke_editor_save_fields() -> EditorSaveLogFields {
    EditorSaveLogFields {
        session_id: "capture-0".to_string(),
        handwriting_markdown_len: 10,
        ai_transcription_markdown_len: 20,
        jsonl_record_count: 1,
    }
}

pub(super) fn emit_release_logging_smoke_events() {
    let capture = TracingCaptureObservability;
    capture.log_phase_transition(CapturePhase::Idle);
    capture.log_phase_transition(CapturePhase::Capturing);
    capture.log_buffer_drop(3);
    capture.log_rt_callback_max_us(1_200);

    let transcribe = TracingTranscribeObservability;
    emit_transcribe_probe(
        &transcribe,
        TranscribePhase::Ready,
        (1, 4),
        2,
        42,
        "smoke test",
    );

    let editor = TracingEditorObservability;
    log_editor_save_lifecycle(&editor, &smoke_editor_save_fields());
}

pub(super) fn emit_release_logging_privacy_events(
    handwriting: &str,
    transcript: &str,
    pcm_marker: &str,
    device_name: &str,
) {
    let capture = TracingCaptureObservability;
    capture.log_phase_transition(CapturePhase::Capturing);
    capture.log_buffer_drop(1);
    capture.log_rt_callback_max_us(900);
    capture.log_stream_open_failure(
        "mic",
        &CaptureError::Internal {
            detail: device_name.to_string(),
        },
        "capture-correlation-6.3",
    );

    let transcribe = TracingTranscribeObservability;
    emit_transcribe_probe(
        &transcribe,
        TranscribePhase::Transcribing,
        (10, 12),
        1,
        55,
        &format!("{pcm_marker} {transcript}"),
    );

    let editor = TracingEditorObservability;
    let fields = save_log_fields("capture-0", handwriting, transcript, 2);
    log_editor_save_lifecycle(&editor, &fields);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_transcribe_probe(
    transcribe: &TracingTranscribeObservability,
    phase: TranscribePhase,
    gaps: (u64, u64),
    block_drops: u64,
    latency_ms: u64,
    error_detail: &str,
) {
    transcribe.log_phase_transition(phase);
    transcribe.log_pcm_sequence_gaps(gaps.0, gaps.1);
    transcribe.log_block_buffer_drop(block_drops);
    transcribe.log_inference_latency(latency_ms);
    transcribe.log_transcribe_error(&TranscribeError::InferenceFailed {
        detail: error_detail.to_string(),
    });
    transcribe.log_stall_detected();
}
