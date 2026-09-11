//! Privacy and masking regression tests for release log persistence.

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::logging::test_support::{
        emit_release_logging_privacy_events, read_log_file, record_release_tracing_session,
        unique_temp_dir,
    };

    const FORBIDDEN_HANDWRITING: &str = "手書き議事録SECRET_6_3";
    const FORBIDDEN_TRANSCRIPT: &str = "転写全文SECRET_6_3";
    const FORBIDDEN_PCM_MARKER: &str = "PCM_SAMPLE_BYTES_AABBCCDD1122";
    const FORBIDDEN_DEVICE_NAME: &str = "Yeti Stereo Microphone";

    #[test]
    fn release_log_omits_forbidden_privacy_content() {
        let app_data = unique_temp_dir("privacy", "privacy-regression");
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

    fn record_privacy_probe_session(app_data: &Path, run_session_id: &str) -> std::path::PathBuf {
        record_release_tracing_session(app_data, run_session_id, emit_privacy_probe_events)
    }

    fn emit_privacy_probe_events() {
        emit_release_logging_privacy_events(
            FORBIDDEN_HANDWRITING,
            FORBIDDEN_TRANSCRIPT,
            FORBIDDEN_PCM_MARKER,
            FORBIDDEN_DEVICE_NAME,
        );
    }

    fn assert_log_omits_forbidden_content(contents: &str, forbidden: &[&str]) {
        for needle in forbidden {
            assert!(
                !contents.contains(needle),
                "release log must omit forbidden content {needle:?}"
            );
        }
    }
}
