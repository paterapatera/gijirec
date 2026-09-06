//! Privacy and masking regression tests for release log persistence.

#[cfg(test)]
mod tests {
    use super::super::build_release_file_tracing_stack;
    use crate::capture_observability::TracingCaptureObservability;
    use crate::editor_observability::TracingEditorObservability;
    use crate::transcribe_observability::TracingTranscribeObservability;
    use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
    use gijirec_presentation::domain::transcribe::{TranscribeError, TranscribePhase};
    use gijirec_presentation::editor::observability::{
        EditorObservability, EditorSaveCompletion, save_log_fields,
    };
    use gijirec_presentation::tauri::observability::{CaptureObservability, init_session_id};
    use gijirec_presentation::transcribe::observability::TranscribeObservability;
    use std::path::{Path, PathBuf};

    const FORBIDDEN_HANDWRITING: &str = "手書き議事録SECRET_6_3";
    const FORBIDDEN_TRANSCRIPT: &str = "転写全文SECRET_6_3";
    const FORBIDDEN_PCM_MARKER: &str = "PCM_SAMPLE_BYTES_AABBCCDD1122";
    const FORBIDDEN_DEVICE_NAME: &str = "Yeti Stereo Microphone";

    #[test]
    fn release_log_omits_forbidden_privacy_content() {
        let app_data = unique_temp_dir("privacy-regression");
        let run_id = "20260906T074500Z-capture-0";
        let log_path = record_privacy_probe_session(&app_data, run_id);
        let contents = read_log_file(&log_path);

        assert!(
            log_path.is_file(),
            "gijirec.log must exist after session start"
        );
        assert!(
            contents.contains("gijirec_capture")
                && contents.contains("gijirec_transcribe")
                && contents.contains("gijirec_editor"),
            "expected observability categories in log: {contents}"
        );
        assert!(
            contents.contains("capture_phase")
                && contents.contains("error_code")
                && contents.contains("session_id"),
            "expected allowed diagnostic fields in log: {contents}"
        );
        assert!(
            contents.contains("transcribe_stall_detected"),
            "expected stall diagnostic in log: {contents}"
        );

        assert_log_omits_forbidden_content(
            &contents,
            &[
                FORBIDDEN_HANDWRITING,
                FORBIDDEN_TRANSCRIPT,
                FORBIDDEN_PCM_MARKER,
                FORBIDDEN_DEVICE_NAME,
                "handwriting_markdown=",
                "ai_transcription_markdown=",
                "jsonl_body",
            ],
        );
    }

    #[test]
    fn release_logging_host_has_no_http_client_usage() {
        let sources = [
            include_str!("mod.rs"),
            include_str!("cli.rs"),
            include_str!("persistence.rs"),
            include_str!("tracing_init.rs"),
            include_str!("make_writer.rs"),
            include_str!("smoke.rs"),
            include_str!("degrade.rs"),
        ];
        for source in sources {
            assert!(
                !source.contains("reqwest"),
                "release logging must not use reqwest"
            );
            assert!(
                !source.contains("ureq::"),
                "release logging must not use ureq"
            );
            assert!(
                !source.contains("hyper::"),
                "release logging must not use hyper client"
            );
        }

        let manifest = include_str!("../../Cargo.toml");
        assert!(
            !manifest.contains("reqwest")
                && !manifest.contains("ureq")
                && !manifest.contains("hyper"),
            "host manifest must not add HTTP client deps for release logging"
        );
    }

    fn record_privacy_probe_session(app_data: &Path, run_session_id: &str) -> PathBuf {
        let stack =
            build_release_file_tracing_stack(app_data, run_session_id).expect("tracing stack");
        let log_path = stack.log_path().to_path_buf();
        let (_, guard) = stack.run_with_default(|| {
            init_session_id();
            emit_privacy_probe_events();
        });
        drop(guard);
        log_path
    }

    fn emit_privacy_probe_events() {
        let capture = TracingCaptureObservability;
        capture.log_phase_transition(CapturePhase::Capturing);
        capture.log_buffer_drop(1);
        capture.log_rt_callback_max_us(900);
        capture.log_stream_open_failure(
            "mic",
            &CaptureError::Internal {
                detail: FORBIDDEN_DEVICE_NAME.to_string(),
            },
            "capture-correlation-6.3",
        );

        let transcribe = TracingTranscribeObservability;
        transcribe.log_phase_transition(TranscribePhase::Transcribing);
        transcribe.log_pcm_sequence_gaps(10, 12);
        transcribe.log_block_buffer_drop(1);
        transcribe.log_inference_latency(55);
        transcribe.log_transcribe_error(&TranscribeError::InferenceFailed {
            detail: format!("{FORBIDDEN_PCM_MARKER} {FORBIDDEN_TRANSCRIPT}"),
        });
        transcribe.log_stall_detected();

        let editor = TracingEditorObservability;
        let fields = save_log_fields("capture-0", FORBIDDEN_HANDWRITING, FORBIDDEN_TRANSCRIPT, 2);
        let completion = EditorSaveCompletion {
            success: true,
            files_written_count: 1,
            error_code: None,
        };
        editor.log_save_started(&fields);
        editor.log_save_completed(&fields, &completion);
    }

    fn assert_log_omits_forbidden_content(contents: &str, forbidden: &[&str]) {
        for needle in forbidden {
            assert!(
                !contents.contains(needle),
                "release log must omit forbidden content {needle:?}"
            );
        }
    }

    fn read_log_file(log_path: &Path) -> String {
        std::fs::read_to_string(log_path).unwrap_or_default()
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-logging-privacy-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
}
