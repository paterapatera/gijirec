//! Integration smoke tests for release logging observability categories.

#[cfg(test)]
mod tests {
    use super::super::{
        ReleaseLogConfig, apply_persistence_if_allowed, build_release_file_tracing_stack, cli,
        run_session_id, setup_release_file_logging,
    };
    use crate::capture_observability::TracingCaptureObservability;
    use crate::editor_observability::TracingEditorObservability;
    use crate::transcribe_observability::TracingTranscribeObservability;
    use gijirec_presentation::domain::audio::CapturePhase;
    use gijirec_presentation::domain::transcribe::{TranscribeError, TranscribePhase};
    use gijirec_presentation::editor::observability::{
        EditorObservability, EditorSaveCompletion, EditorSaveLogFields,
    };
    use gijirec_presentation::tauri::observability::{CaptureObservability, init_session_id};
    use gijirec_presentation::transcribe::observability::TranscribeObservability;
    use std::path::{Path, PathBuf};

    #[test]
    fn release_log_enabled_session_writes_observability_categories_to_gijirec_log() {
        let app_data = unique_temp_dir("enabled-session");
        let run_id = run_session_id("capture-0");
        let log_path = record_session_observability_smoke(&app_data, &run_id);
        let contents = read_log_file(&log_path);

        assert!(
            log_path.is_file(),
            "gijirec.log must exist after session start"
        );
        assert!(
            contents.contains("gijirec_capture") && contents.contains("capture_phase"),
            "expected capture phase transition lines in log: {contents}"
        );
        assert!(
            contents.contains("capturing"),
            "expected capture phase value in log: {contents}"
        );
        assert!(
            contents.contains("gijirec_transcribe")
                && contents.contains("transcribe_phase")
                && contents.contains("error_code")
                && contents.contains("session_id"),
            "expected transcribe fields in log: {contents}"
        );
        assert!(
            contents.contains("transcribe_pcm_sequence_gaps")
                && contents.contains("transcribe_block_buffer_drops")
                && contents.contains("transcribe_inference_latency_ms")
                && contents.contains("transcribe_stall_detected"),
            "expected transcribe gap/drop/latency/stall metrics in log: {contents}"
        );
        assert!(
            contents.contains("capture_buffer_drops_total")
                && contents.contains("capture_rt_callback_max_us"),
            "expected capture buffer drop and latency metrics in log: {contents}"
        );
        assert!(
            contents.contains("gijirec_editor")
                && contents.contains("editor_save_started")
                && contents.contains("editor_save_completed"),
            "expected editor save outcome indicators in log: {contents}"
        );

        let latest = app_data.join("logs").join("latest-session.txt");
        assert_eq!(
            std::fs::read_to_string(latest).expect("read latest-session"),
            run_id
        );
    }

    #[test]
    fn disabled_release_log_config_does_not_create_sessions_tree() {
        let config = ReleaseLogConfig {
            file_logging_enabled: false,
        };
        let app_data = unique_temp_dir("disabled-release-log");
        let run_id = "20260906T074500Z-capture-0";

        apply_persistence_if_allowed(&app_data, &config, run_id).expect("disabled apply is no-op");
        assert!(setup_release_file_logging(&app_data, &config, run_id).is_none());
        assert!(
            !app_data.join("logs").join("sessions").exists(),
            "release without --log must not create logs/sessions (debug uses file_logging_enabled=false; release matrix validated separately)"
        );
    }

    #[test]
    fn file_logging_enabled_for_matrix_documents_release_noop_without_log_flag() {
        assert!(!cli::file_logging_enabled_for(true, false));
        assert!(!cli::file_logging_enabled_for(false, true));
        assert!(cli::file_logging_enabled_for(true, true));
        assert!(!cli::file_logging_enabled_for(false, false));
    }

    #[test]
    fn multiple_sessions_use_independent_log_files() {
        let app_data = unique_temp_dir("multi-session");
        let run_a = "20260906T070000Z-capture-0";
        let run_b = "20260906T080000Z-capture-1";

        let log_a = record_session_observability_smoke(&app_data, run_a);
        let log_b = record_session_observability_smoke(&app_data, run_b);

        assert_ne!(log_a, log_b);
        assert!(log_a.is_file());
        assert!(log_b.is_file());
        assert_eq!(
            std::fs::read_to_string(app_data.join("logs").join("latest-session.txt"))
                .expect("read latest-session"),
            run_b
        );
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_cli_without_log_does_not_create_sessions_tree() {
        use super::super::parse_release_log_config;

        let config = parse_release_log_config(["gijirec"]);
        assert!(!config.file_logging_enabled);

        let app_data = unique_temp_dir("release-cli-no-log");
        let run_id = run_session_id("capture-0");

        apply_persistence_if_allowed(&app_data, &config, &run_id).expect("release no-op apply");
        assert!(setup_release_file_logging(&app_data, &config, &run_id).is_none());
        assert!(
            !app_data.join("logs").join("sessions").exists(),
            "release CLI without --log must not create logs/sessions"
        );
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_cli_with_log_writes_capture_phase_via_setup() {
        use super::super::parse_release_log_config;
        use super::super::persistence_side_effects_allowed;

        let config = parse_release_log_config(["gijirec", "--log"]);
        assert!(config.file_logging_enabled);
        assert!(
            persistence_side_effects_allowed(&config),
            "release --log must enable persistence"
        );

        let app_data = unique_temp_dir("release-cli-with-log");
        let run_id = run_session_id("capture-0");
        apply_persistence_if_allowed(&app_data, &config, &run_id).expect("release apply");

        // Avoid set_global_default: other tests in this process may already own the dispatcher.
        let stack = build_release_file_tracing_stack(&app_data, &run_id).expect("tracing stack");
        let (_, guard) = stack.run_with_default(emit_capture_phase_line);
        drop(guard);

        let contents = read_log_file(&session_log_path_for(&app_data, &run_id));
        assert!(
            contents.contains("gijirec_capture")
                && contents.contains("capture_phase")
                && contents.contains("capturing"),
            "release --log must persist capture phase lines: {contents}"
        );
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_config_with_log_writes_capture_phase_via_tracing_stack() {
        use super::super::persistence_side_effects_allowed;

        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        assert!(persistence_side_effects_allowed(&config));

        let app_data = unique_temp_dir("release-config-with-log");
        let run_id = run_session_id("capture-0");
        apply_persistence_if_allowed(&app_data, &config, &run_id).expect("release apply");

        let stack = build_release_file_tracing_stack(&app_data, &run_id).expect("tracing stack");
        let log_path = stack.log_path().to_path_buf();
        let (_, guard) = stack.run_with_default(emit_capture_phase_line);
        drop(guard);

        let contents = read_log_file(&log_path);
        assert!(
            contents.contains("gijirec_capture")
                && contents.contains("capture_phase")
                && contents.contains("capturing"),
            "release config with --log must persist capture phase lines: {contents}"
        );
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_build_select_subscriber_mode_matrix() {
        use super::super::persistence_side_effects_allowed;
        use super::super::select_subscriber_mode;
        use super::super::tracing_init::TracingSubscriberMode;

        let without_log = ReleaseLogConfig {
            file_logging_enabled: false,
        };
        let with_log = ReleaseLogConfig {
            file_logging_enabled: true,
        };

        assert_eq!(
            select_subscriber_mode(&without_log),
            TracingSubscriberMode::ReleaseNoop
        );
        assert_eq!(
            select_subscriber_mode(&with_log),
            TracingSubscriberMode::ReleaseFileLogging
        );
        assert!(!persistence_side_effects_allowed(&without_log));
        assert!(persistence_side_effects_allowed(&with_log));
    }

    #[cfg(not(debug_assertions))]
    fn emit_capture_phase_line() {
        TracingCaptureObservability.log_phase_transition(CapturePhase::Capturing);
    }

    fn record_session_observability_smoke(app_data: &Path, run_session_id: &str) -> PathBuf {
        let stack =
            build_release_file_tracing_stack(app_data, run_session_id).expect("tracing stack");
        let log_path = stack.log_path().to_path_buf();
        let (_, guard) = stack.run_with_default(|| {
            init_session_id();
            emit_contract_observability_events();
        });
        drop(guard);
        log_path
    }

    fn emit_contract_observability_events() {
        let capture = TracingCaptureObservability;
        capture.log_phase_transition(CapturePhase::Idle);
        capture.log_phase_transition(CapturePhase::Capturing);
        capture.log_buffer_drop(3);
        capture.log_rt_callback_max_us(1_200);

        let transcribe = TracingTranscribeObservability;
        transcribe.log_phase_transition(TranscribePhase::Ready);
        transcribe.log_pcm_sequence_gaps(1, 4);
        transcribe.log_block_buffer_drop(2);
        transcribe.log_inference_latency(42);
        transcribe.log_transcribe_error(&TranscribeError::InferenceFailed {
            detail: "smoke test".to_string(),
        });
        transcribe.log_stall_detected();

        let editor = TracingEditorObservability;
        let fields = EditorSaveLogFields {
            session_id: "capture-0".to_string(),
            handwriting_markdown_len: 10,
            ai_transcription_markdown_len: 20,
            jsonl_record_count: 1,
        };
        let completion = EditorSaveCompletion {
            success: true,
            files_written_count: 1,
            error_code: None,
        };
        editor.log_save_started(&fields);
        editor.log_save_completed(&fields, &completion);
    }

    #[cfg(not(debug_assertions))]
    fn session_log_path_for(app_data: &Path, run_id: &str) -> PathBuf {
        app_data
            .join("logs")
            .join("sessions")
            .join(run_id)
            .join("gijirec.log")
    }

    fn read_log_file(log_path: &Path) -> String {
        std::fs::read_to_string(log_path).unwrap_or_default()
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-release-logging-smoke-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
}
