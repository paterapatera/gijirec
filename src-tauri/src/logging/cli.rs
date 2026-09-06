//! CLI parsing for `--log` (ReleaseLogCli).

use super::ReleaseLogConfig;

const LOG_FLAG: &str = "--log";

/// Returns whether the canonical `--log` flag appears in `args`.
pub fn log_flag_present<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == LOG_FLAG)
}

/// Exposed for integration smoke tests documenting release CLI/build matrix semantics.
pub(super) fn file_logging_enabled_for(release_build: bool, log_flag_present: bool) -> bool {
    release_build && log_flag_present
}

/// Parses CLI args into release logging configuration for the current build.
pub fn parse_release_log_config<I, S>(args: I) -> ReleaseLogConfig
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let log_flag_present = log_flag_present(args);
    ReleaseLogConfig {
        file_logging_enabled: file_logging_enabled_for(
            cfg!(not(debug_assertions)),
            log_flag_present,
        ),
    }
}

/// Parses `std::env::args()` at application entry.
pub fn parse_release_log_config_from_env() -> ReleaseLogConfig {
    parse_release_log_config(std::env::args())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_flag_present_when_log_option_specified() {
        assert!(log_flag_present(["gijirec", "--log"]));
    }

    #[test]
    fn log_flag_absent_without_log_option() {
        assert!(!log_flag_present(["gijirec"]));
    }

    #[test]
    fn log_flag_ignores_unknown_options() {
        assert!(log_flag_present(["gijirec", "--verbose", "--log", "extra"]));
        assert!(!log_flag_present(["gijirec", "--unknown", "-v"]));
    }

    #[test]
    fn file_logging_enabled_for_all_build_and_flag_combinations() {
        assert!(!file_logging_enabled_for(false, false));
        assert!(!file_logging_enabled_for(false, true));
        assert!(!file_logging_enabled_for(true, false));
        assert!(file_logging_enabled_for(true, true));
    }

    #[test]
    fn parse_release_log_config_matches_current_build_cfg() {
        let with_log = parse_release_log_config(["gijirec", "--log"]);
        let without_log = parse_release_log_config(["gijirec"]);

        if cfg!(debug_assertions) {
            assert!(!with_log.file_logging_enabled);
            assert!(!without_log.file_logging_enabled);
        } else {
            assert!(with_log.file_logging_enabled);
            assert!(!without_log.file_logging_enabled);
        }
    }

    #[test]
    fn parse_release_log_config_ignores_unknown_options() {
        let config = parse_release_log_config(["gijirec", "--future-tauri-flag", "--log"]);
        let expected = file_logging_enabled_for(cfg!(not(debug_assertions)), true);
        assert_eq!(config.file_logging_enabled, expected);
    }
}
