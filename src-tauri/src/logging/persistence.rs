//! Session log path resolution and tracing-appender wiring.

use super::{ReleaseLogConfig, is_filesystem_safe_run_session_id, release_file_logging_active};
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// Error returned when release log persistence cannot be initialized.
#[derive(Debug)]
pub struct ReleaseLogInitError {
    message: String,
}

impl ReleaseLogInitError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ReleaseLogInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ReleaseLogInitError {}

/// Guard that keeps the non-blocking appender worker alive for the session.
pub struct ReleaseLogLayerGuard {
    _non_blocking: Option<NonBlocking>,
    _worker_guard: Option<WorkerGuard>,
}

/// Tauri-managed state holding the non-blocking appender worker guard.
pub struct LogGuardState {
    _worker_guard: WorkerGuard,
}

impl LogGuardState {
    pub fn new(worker_guard: WorkerGuard) -> Self {
        Self {
            _worker_guard: worker_guard,
        }
    }
}

fn validate_run_session_id(run_session_id: &str) -> Result<(), ReleaseLogInitError> {
    if is_filesystem_safe_run_session_id(run_session_id) {
        Ok(())
    } else {
        Err(ReleaseLogInitError {
            message: format!("invalid run_session_id for release logging: {run_session_id}"),
        })
    }
}

fn io_error(context: &str, err: std::io::Error) -> ReleaseLogInitError {
    ReleaseLogInitError {
        message: format!("{context}: {err}"),
    }
}

/// Resolves `{app_data_dir}/logs/latest-session.txt`.
pub(super) fn latest_session_pointer_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("logs").join("latest-session.txt")
}

/// Resolves `{app_data_dir}/logs/sessions/{run_session_id}/gijirec.log`.
pub(super) fn session_log_path(app_data_dir: &Path, run_session_id: &str) -> PathBuf {
    validate_run_session_id(run_session_id).expect("run_session_id must be filesystem-safe");
    app_data_dir
        .join("logs")
        .join("sessions")
        .join(run_session_id)
        .join("gijirec.log")
}

/// Resolves `{app_data_dir}/logs/sessions/{run_session_id}/`.
pub(super) fn session_log_dir(app_data_dir: &Path, run_session_id: &str) -> PathBuf {
    session_log_path(app_data_dir, run_session_id)
        .parent()
        .expect("session log path has parent directory")
        .to_path_buf()
}

/// Creates the session log directory and overwrites `logs/latest-session.txt`.
pub(crate) fn create_session_log_files(
    app_data_dir: &Path,
    run_session_id: &str,
) -> Result<(PathBuf, PathBuf), ReleaseLogInitError> {
    validate_run_session_id(run_session_id)?;

    let session_dir = session_log_dir(app_data_dir, run_session_id);
    std::fs::create_dir_all(&session_dir)
        .map_err(|err| io_error("failed to create session log directory", err))?;

    let log_path = session_log_path(app_data_dir, run_session_id);
    let latest_path = latest_session_pointer_path(app_data_dir);
    if let Some(parent) = latest_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| io_error("failed to create logs directory", err))?;
    }
    std::fs::write(&latest_path, run_session_id.as_bytes())
        .map_err(|err| io_error("failed to write latest-session pointer", err))?;

    Ok((session_dir, log_path))
}

/// Builds a non-blocking file appender that appends to a single `gijirec.log` file.
pub(crate) fn build_nonblocking_appender(
    session_dir: &Path,
) -> Result<(NonBlocking, WorkerGuard), ReleaseLogInitError> {
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::NEVER)
        .filename_prefix("gijirec")
        .filename_suffix("log")
        .build(session_dir)
        .map_err(|err| ReleaseLogInitError {
            message: format!(
                "failed to build release log appender in {}: {err}",
                session_dir.display()
            ),
        })?;

    Ok(tracing_appender::non_blocking(file_appender))
}

/// Prepares session log files and a non-blocking appender without installing tracing layers.
pub fn prepare_release_file_layer(
    app_data_dir: &Path,
    run_session_id: &str,
) -> Result<(NonBlocking, WorkerGuard, PathBuf), ReleaseLogInitError> {
    let (session_dir, log_path) = create_session_log_files(app_data_dir, run_session_id)?;
    let (non_blocking, worker_guard) = build_nonblocking_appender(&session_dir)?;
    Ok((non_blocking, worker_guard, log_path))
}

/// Initializes the release log persistence layer when enabled.
pub fn init_release_log_layer(
    app_data_dir: &Path,
    config: &ReleaseLogConfig,
    run_session_id: &str,
) -> Result<ReleaseLogLayerGuard, ReleaseLogInitError> {
    if !release_file_logging_active(config) {
        return Ok(ReleaseLogLayerGuard {
            _non_blocking: None,
            _worker_guard: None,
        });
    }

    let (non_blocking, worker_guard, _log_path) =
        prepare_release_file_layer(app_data_dir, run_session_id)?;

    Ok(ReleaseLogLayerGuard {
        _non_blocking: Some(non_blocking),
        _worker_guard: Some(worker_guard),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gijirec-persistence-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn latest_session_pointer_path_matches_contract() {
        let app_data = Path::new("/app/data");
        assert_eq!(
            latest_session_pointer_path(app_data),
            app_data.join("logs").join("latest-session.txt")
        );
    }

    #[test]
    fn create_session_log_files_returns_error_for_invalid_run_session_id() {
        let app_data = unique_temp_dir("invalid-run-id");
        let result = create_session_log_files(&app_data, "../escape");
        assert!(result.is_err());
    }
}
