//! Integration tests for release log persistence failure degrade paths.

#[cfg(test)]
mod tests {
    use super::super::make_writer::{AlwaysFailingMakeWriter, ReleaseLogMakeWriter};
    use super::super::persistence::create_session_log_files;
    use super::super::tracing_init::build_release_file_tracing_stack_with_make_writer;
    use super::super::{
        RELEASE_LOG_TARGET, ReleaseLogConfig, ReleaseLogInitError,
        build_release_file_tracing_stack, degrade_after_persistence_failure,
        persistence_failure_warn_metadata, setup_release_file_logging,
        try_setup_release_file_logging_unchecked,
    };
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

    #[test]
    fn startup_unwritable_app_data_degrades_without_blocking_setup() {
        let app_data = path_blocked_by_file();
        let run_id = "20260906T074500Z-capture-0";

        let persist_err = create_session_log_files(&app_data, run_id)
            .expect_err("app data blocked by file must fail persistence");
        assert!(degrade_after_persistence_failure(&persist_err).is_none());

        let attach_err = match build_release_file_tracing_stack(&app_data, run_id) {
            Err(err) => err,
            Ok(_) => panic!("unwritable app data must fail attach"),
        };
        assert!(degrade_after_persistence_failure(&attach_err).is_none());

        assert!(
            try_setup_release_file_logging_unchecked(&app_data, run_id).is_none(),
            "unchecked setup must degrade without blocking startup"
        );

        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        if cfg!(debug_assertions) {
            assert!(
                setup_release_file_logging(&app_data, &config, run_id).is_none(),
                "debug profile gate returns None before persistence is attempted"
            );
        }
    }

    #[test]
    fn during_session_write_failure_surfaces_persistence_warn_once() {
        let app_data = unique_temp_dir("session-write-fail");
        let run_id = "20260906T074500Z-capture-0";
        let make_writer = ReleaseLogMakeWriter::new(AlwaysFailingMakeWriter);
        let stack =
            build_release_file_tracing_stack_with_make_writer(&app_data, run_id, make_writer)
                .expect("session files created before write failure");

        let surfaced = stack.persistence_failure_handle();
        assert!(!surfaced.load(Ordering::SeqCst));

        let (_, guard) = stack.run_with_default(|| {
            tracing::info!(
                target: "gijirec_capture",
                capture_phase = "capturing",
                "started"
            );
        });
        drop(guard);

        assert!(
            surfaced.load(Ordering::SeqCst),
            "write failure must surface persistence diagnostics once"
        );
        let metadata = persistence_failure_warn_metadata(&ReleaseLogInitError::new(
            "release log write failed: disk full during session",
        ));
        assert_eq!(metadata.target, RELEASE_LOG_TARGET);
        assert!(metadata.release_log_persistence_failed);
        assert!(metadata.error.contains("disk full during session"));
    }

    #[test]
    #[cfg(windows)]
    fn during_session_readonly_log_prevents_reopening_appender() {
        let app_data = unique_temp_dir("session-readonly");
        let run_id = "20260906T074500Z-capture-0";
        let stack = build_release_file_tracing_stack(&app_data, run_id).expect("logging started");
        let log_path = stack.log_path().to_path_buf();

        let (_, guard) = stack.run_with_default(|| {
            tracing::info!(target: "gijirec_capture", capture_phase = "capturing", "first");
        });
        drop(guard);
        assert!(log_path.is_file());

        let mut perms = std::fs::metadata(&log_path)
            .expect("log metadata")
            .permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&log_path, perms).expect("set readonly");

        let reopen_err = match build_release_file_tracing_stack(&app_data, run_id) {
            Err(err) => err,
            Ok(_) => return,
        };
        assert!(degrade_after_persistence_failure(&reopen_err).is_none());
        let metadata = persistence_failure_warn_metadata(&reopen_err);
        assert_eq!(metadata.target, RELEASE_LOG_TARGET);
        assert!(metadata.release_log_persistence_failed);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_setup_on_unwritable_app_data_returns_none_without_panic() {
        use super::super::apply_persistence_if_allowed;

        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        let app_data = path_blocked_by_file();
        let run_id = "20260906T074500Z-capture-0";

        assert!(apply_persistence_if_allowed(&app_data, &config, run_id).is_err());
        assert!(setup_release_file_logging(&app_data, &config, run_id).is_none());
        assert!(try_setup_release_file_logging_unchecked(&app_data, run_id).is_none());
    }

    fn path_blocked_by_file() -> PathBuf {
        let dir = unique_temp_dir("blocked-app-data");
        let blocker = dir.join("app_data");
        std::fs::write(&blocker, b"not-a-directory").expect("write blocker file");
        blocker
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-logging-degrade-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
