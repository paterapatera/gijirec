//! Release-build file logging (host-only).

mod cli;
mod make_writer;
mod persistence;
mod tracing_init;

#[cfg(test)]
mod degrade;
#[cfg(test)]
mod privacy;
#[cfg(test)]
mod smoke;
#[cfg(test)]
mod test_support;

pub use cli::{log_flag_present, parse_release_log_config, parse_release_log_config_from_env};
pub use persistence::{
    LogGuardState, ReleaseLogInitError, ReleaseLogLayerGuard, init_release_log_layer,
    prepare_release_file_layer,
};
pub use tracing_init::{
    PersistenceFailureWarnMetadata, RELEASE_LOG_TARGET, ReleaseFileTracingStack,
    TracingSubscriberMode, apply_persistence_if_allowed, attach_release_file_layer,
    build_release_file_tracing_stack, default_env_filter, defers_subscriber_init_to_setup,
    degrade_after_persistence_failure, install_global_subscriber,
    persistence_failure_warn_metadata, persistence_side_effects_allowed, select_subscriber_mode,
    setup_release_file_logging, surface_persistence_failure,
    try_setup_release_file_logging_unchecked,
};

/// Whether release file logging is requested via CLI (`--log`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseLogConfig {
    pub file_logging_enabled: bool,
}

/// Returns whether release file logging should be active for the current build and config.
pub fn release_file_logging_active(config: &ReleaseLogConfig) -> bool {
    cfg!(not(debug_assertions)) && config.file_logging_enabled
}

/// Builds `{YYYYMMDDTHHMMSSZ}-{capture_session_suffix}` for the session log directory.
pub fn run_session_id(capture_session_suffix: &str) -> String {
    let timestamp = utc_timestamp_compact(std::time::SystemTime::now());
    let suffix = sanitize_session_suffix(capture_session_suffix);
    format!("{timestamp}-{suffix}")
}

/// Sanitizes a capture session suffix for use in a filesystem directory name.
pub(super) fn sanitize_session_suffix(raw: &str) -> String {
    const FORBIDDEN: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let mut sanitized = String::with_capacity(raw.len());
    let mut last_was_dash = false;

    for c in raw.chars() {
        let allowed = (c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !c.is_control()
            && !FORBIDDEN.contains(&c);
        if allowed {
            sanitized.push(c);
            last_was_dash = false;
        } else if !sanitized.is_empty() && !last_was_dash {
            sanitized.push('-');
            last_was_dash = true;
        }
    }

    let trimmed = sanitized
        .trim_end_matches(['.', ' '])
        .trim_matches('-')
        .to_string();
    if trimmed.is_empty() {
        return "session".to_string();
    }
    if is_windows_reserved_name(&trimmed) {
        return format!("_{trimmed}");
    }
    trimmed
}

pub(super) fn is_filesystem_safe_run_session_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    const FORBIDDEN: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    if id.chars().any(|c| FORBIDDEN.contains(&c) || c.is_control()) {
        return false;
    }
    let Some((timestamp, suffix)) = id.split_once('-') else {
        return false;
    };
    if timestamp.len() != 16 || !timestamp.ends_with('Z') || !timestamp.contains('T') {
        return false;
    }
    !suffix.is_empty()
        && !suffix
            .chars()
            .any(|c| FORBIDDEN.contains(&c) || c.is_control())
        && !is_windows_reserved_name(suffix)
        && !suffix.contains("..")
}

fn is_windows_reserved_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM0"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT0"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn utc_timestamp_compact(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let time_of_day = secs % 86_400;
    let (year, month, day) = civil_from_days(secs / 86_400);
    let hour = time_of_day / 3_600;
    let minute = (time_of_day % 3_600) / 60;
    let second = time_of_day % 60;
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

fn civil_from_days(days_since_epoch: u64) -> (u32, u32, u32) {
    let z = days_since_epoch as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_097 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mp < 10 { year } else { year + 1 };
    (year as u32, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn release_log_config_exposes_file_logging_enabled() {
        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        assert!(config.file_logging_enabled);
    }

    #[test]
    fn run_session_id_appends_capture_suffix() {
        let id = run_session_id("capture-0");
        assert!(id.ends_with("-capture-0"));
        assert_eq!(id.len(), "20260906T074500Z-capture-0".len());
        assert!(id.contains('T'));
    }

    #[test]
    fn run_session_id_matches_contract_timestamp_format() {
        let id = run_session_id("capture-0");
        let (timestamp, suffix) = id
            .split_once('-')
            .expect("run_session_id must contain hyphen separator");
        assert_eq!(suffix, "capture-0");
        assert!(
            timestamp.len() == 16 && timestamp.ends_with('Z') && timestamp.contains('T'),
            "expected YYYYMMDDTHHMMSSZ prefix, got {timestamp}"
        );
        assert!(timestamp[..8].chars().all(|c| c.is_ascii_digit()));
        let time_part = &timestamp[9..15];
        assert!(time_part.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn run_session_id_excludes_forbidden_characters_from_unsafe_suffix() {
        let unsafe_suffixes = [
            "capture/0",
            "capture\\0",
            "capture:0",
            "capture<0>",
            "capture|0",
            "capture\"0",
            "capture?0",
            "capture*0",
            "..\\etc",
            "CON",
        ];
        for suffix in unsafe_suffixes {
            let id = run_session_id(suffix);
            assert!(
                is_filesystem_safe_run_session_id(&id),
                "run_session_id({suffix:?}) produced unsafe id {id:?}"
            );
            assert!(
                !id.contains('/') && !id.contains('\\'),
                "run_session_id({suffix:?}) must not contain path separators: {id}"
            );
        }
    }

    #[test]
    fn run_session_id_preserves_safe_capture_suffix() {
        assert!(run_session_id("capture-0").ends_with("-capture-0"));
        assert!(run_session_id("capture-42").ends_with("-capture-42"));
    }

    #[test]
    fn session_log_path_stays_under_app_data_dir() {
        let app_data = Path::new("C:\\Users\\me\\AppData\\Roaming\\com.gijirec.desktop");
        let run_id = run_session_id("capture-0");
        let path = persistence::session_log_path(app_data, &run_id);
        assert!(path.starts_with(app_data));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("gijirec.log")
        );
        assert!(
            path.components().count() >= 5,
            "expected logs/sessions/{{run_id}}/gijirec.log under app data"
        );
    }

    fn is_filesystem_safe_run_session_id(id: &str) -> bool {
        super::is_filesystem_safe_run_session_id(id)
    }

    #[test]
    fn sanitize_session_suffix_replaces_path_separators() {
        assert_eq!(sanitize_session_suffix("capture/0"), "capture-0");
        assert_eq!(sanitize_session_suffix("capture\\0"), "capture-0");
    }

    #[test]
    fn sanitize_session_suffix_escapes_windows_reserved_names() {
        assert_eq!(sanitize_session_suffix("CON"), "_CON");
        assert_ne!(sanitize_session_suffix("CON"), "CON");
    }

    #[test]
    fn session_log_path_matches_contract_layout() {
        let app_data = Path::new("/app/data");
        let run_id = "20260906T074500Z-capture-0";
        let path = persistence::session_log_path(app_data, run_id);
        assert_eq!(
            path,
            app_data
                .join("logs")
                .join("sessions")
                .join(run_id)
                .join("gijirec.log")
        );
    }

    #[test]
    fn init_release_log_layer_skips_when_disabled() {
        let app_data = std::env::temp_dir().join("gijirec-logging-test-disabled");
        let config = ReleaseLogConfig {
            file_logging_enabled: false,
        };
        let result = init_release_log_layer(&app_data, &config, "20260906T074500Z-capture-0");
        assert!(result.is_ok());
    }

    #[test]
    fn create_session_log_files_writes_latest_session_pointer() {
        let app_data = unique_temp_dir("create-session-log-files");
        let run_id = "20260906T074500Z-capture-0";

        let (session_dir, log_path) =
            persistence::create_session_log_files(&app_data, run_id).expect("create session files");

        assert!(session_dir.is_dir());
        assert_eq!(
            log_path,
            session_dir.join("gijirec.log"),
            "session log must be a single gijirec.log file"
        );

        let latest_path = persistence::latest_session_pointer_path(&app_data);
        assert!(latest_path.is_file());
        let latest_contents = std::fs::read_to_string(&latest_path).expect("read latest-session");
        assert_eq!(latest_contents, run_id);
    }

    #[test]
    fn create_session_log_files_overwrites_latest_session_pointer() {
        let app_data = unique_temp_dir("overwrite-latest-session");
        let first = "20260906T070000Z-capture-0";
        let second = "20260906T080000Z-capture-1";

        persistence::create_session_log_files(&app_data, first).expect("first session");
        persistence::create_session_log_files(&app_data, second).expect("second session");

        let latest_path = persistence::latest_session_pointer_path(&app_data);
        let latest_contents = std::fs::read_to_string(&latest_path).expect("read latest-session");
        assert_eq!(latest_contents, second);
    }

    #[test]
    fn build_nonblocking_appender_uses_single_session_log_file() {
        let app_data = unique_temp_dir("nonblocking-appender");
        let run_id = "20260906T074500Z-capture-0";
        let (session_dir, log_path) =
            persistence::create_session_log_files(&app_data, run_id).expect("create session files");

        let (mut writer, guard) =
            persistence::build_nonblocking_appender(&session_dir).expect("build appender");
        use std::io::Write;
        writer
            .write_all(b"release log line\n")
            .expect("write log line");
        drop(guard);

        assert!(log_path.is_file(), "expected gijirec.log to exist");
        let sibling_logs: Vec<_> = std::fs::read_dir(&session_dir)
            .expect("read session dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(
            sibling_logs.len(),
            1,
            "session dir must contain one log file"
        );
        assert_eq!(sibling_logs[0].to_string_lossy(), "gijirec.log");

        let contents = std::fs::read_to_string(&log_path).expect("read gijirec.log");
        assert!(contents.contains("release log line"));
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gijirec-logging-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn release_file_logging_inactive_in_debug_build() {
        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        if cfg!(debug_assertions) {
            assert!(!release_file_logging_active(&config));
        }
    }
}
