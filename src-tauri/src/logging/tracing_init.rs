//! Tracing subscriber mode selection for release logging integration.

use super::ReleaseLogConfig;
use super::make_writer::ReleaseLogMakeWriter;
use super::persistence::{LogGuardState, ReleaseLogInitError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Default `EnvFilter` directive when `RUST_LOG` is unset.
pub(super) const DEFAULT_ENV_FILTER: &str =
    "gijirec_capture=info,gijirec_transcribe=info,gijirec_editor=info,info";

/// Target used for release log persistence failure events.
pub const RELEASE_LOG_TARGET: &str = "gijirec_release_log";

/// Subscriber layout selected from build mode and CLI configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracingSubscriberMode {
    /// Development builds: stdout formatting only; `--log` is ignored.
    DebugStdout,
    /// Release builds without `--log`: registry only, no console or file output.
    ReleaseNoop,
    /// Release builds with `--log`: file layer attached in Tauri setup.
    ReleaseFileLogging,
}

/// Metadata for the persistence failure WARN event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistenceFailureWarnMetadata {
    pub target: &'static str,
    pub release_log_persistence_failed: bool,
    pub error: String,
}

/// Default `EnvFilter` when `RUST_LOG` is unset.
pub fn default_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_ENV_FILTER))
}

/// Chooses the tracing subscriber mode from build profile and CLI config.
pub fn select_subscriber_mode(config: &ReleaseLogConfig) -> TracingSubscriberMode {
    if cfg!(debug_assertions) {
        TracingSubscriberMode::DebugStdout
    } else if config.file_logging_enabled {
        TracingSubscriberMode::ReleaseFileLogging
    } else {
        TracingSubscriberMode::ReleaseNoop
    }
}

/// Whether release log persistence may create files under `app_data_dir`.
pub fn persistence_side_effects_allowed(config: &ReleaseLogConfig) -> bool {
    super::release_file_logging_active(config)
}

/// Whether global subscriber initialization must wait for Tauri setup.
pub fn defers_subscriber_init_to_setup(config: &ReleaseLogConfig) -> bool {
    select_subscriber_mode(config) == TracingSubscriberMode::ReleaseFileLogging
}

/// Creates session log files only when release persistence is allowed for this build/config.
pub fn apply_persistence_if_allowed(
    app_data: &Path,
    config: &ReleaseLogConfig,
    run_session_id: &str,
) -> Result<(), ReleaseLogInitError> {
    if !persistence_side_effects_allowed(config) {
        return Ok(());
    }
    super::persistence::create_session_log_files(app_data, run_session_id).map(|_| ())
}

/// Builds WARN metadata for persistence failure surfacing.
pub fn persistence_failure_warn_metadata(
    err: &ReleaseLogInitError,
) -> PersistenceFailureWarnMetadata {
    PersistenceFailureWarnMetadata {
        target: RELEASE_LOG_TARGET,
        release_log_persistence_failed: true,
        error: err.message().to_string(),
    }
}

/// Emits stderr diagnostics and a tracing WARN for persistence failure.
pub fn surface_persistence_failure(err: &ReleaseLogInitError) {
    let metadata = persistence_failure_warn_metadata(err);
    eprintln!("release log persistence failed: {}", metadata.error);
    tracing::warn!(
        target: RELEASE_LOG_TARGET,
        release_log_persistence_failed = metadata.release_log_persistence_failed,
        error = metadata.error.as_str(),
        "release log persistence failed"
    );
}

fn install_noop_registry_subscriber() {
    let filter = default_env_filter();
    let _ = tracing_subscriber::registry().with(filter).try_init();
}

/// Degrades after persistence failure without blocking application startup.
pub fn degrade_after_persistence_failure(err: &ReleaseLogInitError) -> Option<LogGuardState> {
    install_noop_registry_subscriber();
    surface_persistence_failure(err);
    None
}

/// Installs the global tracing subscriber for modes that do not defer to setup.
pub fn install_global_subscriber(config: &ReleaseLogConfig) {
    if defers_subscriber_init_to_setup(config) {
        return;
    }

    let filter = default_env_filter();
    match select_subscriber_mode(config) {
        TracingSubscriberMode::DebugStdout => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
        TracingSubscriberMode::ReleaseNoop => {
            tracing_subscriber::registry().with(filter).init();
        }
        TracingSubscriberMode::ReleaseFileLogging => {}
    }
}

/// Prepared release file fmt layer stack without installing a global subscriber.
pub struct ReleaseFileTracingStack<S> {
    subscriber: S,
    guard: LogGuardState,
    log_path: PathBuf,
    persistence_failure_surfaced: Arc<AtomicBool>,
}

impl<S> ReleaseFileTracingStack<S> {
    /// Path to the session `gijirec.log` file.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Shared flag set when a write failure is surfaced during this session.
    pub fn persistence_failure_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.persistence_failure_surfaced)
    }
}

impl<S: tracing::Subscriber + Send + Sync + 'static> ReleaseFileTracingStack<S> {
    /// Runs a closure with this stack as the thread-default subscriber (test-friendly).
    pub fn run_with_default<R>(self, f: impl FnOnce() -> R) -> (R, LogGuardState) {
        let guard = self.guard;
        (tracing::subscriber::with_default(self.subscriber, f), guard)
    }
}

impl<S: SubscriberInitExt> ReleaseFileTracingStack<S> {
    /// Installs this stack as the process-global tracing subscriber.
    pub fn install_global(self) -> LogGuardState {
        self.subscriber.init();
        self.guard
    }
}

fn build_file_fmt_subscriber<W>(
    make_writer: ReleaseLogMakeWriter<W>,
) -> impl tracing::Subscriber + Send + Sync + SubscriberInitExt + 'static
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::registry()
        .with(default_env_filter())
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(make_writer)
                .with_ansi(false),
        )
}

fn assemble_release_file_tracing_stack<W>(
    make_writer: ReleaseLogMakeWriter<W>,
    worker_guard: WorkerGuard,
    log_path: PathBuf,
) -> ReleaseFileTracingStack<impl tracing::Subscriber + Send + Sync + SubscriberInitExt + 'static>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let persistence_failure_surfaced = make_writer.surfaced_handle();
    let subscriber = build_file_fmt_subscriber(make_writer);
    ReleaseFileTracingStack {
        subscriber,
        guard: LogGuardState::new(worker_guard),
        log_path,
        persistence_failure_surfaced,
    }
}

/// Builds session files and a file fmt subscriber stack without global init.
pub fn build_release_file_tracing_stack(
    app_data: &Path,
    run_session_id: &str,
) -> Result<
    ReleaseFileTracingStack<impl tracing::Subscriber + Send + Sync + SubscriberInitExt + 'static>,
    ReleaseLogInitError,
> {
    let (non_blocking, worker_guard, log_path) =
        super::persistence::prepare_release_file_layer(app_data, run_session_id)?;
    let make_writer = ReleaseLogMakeWriter::new(non_blocking);
    Ok(assemble_release_file_tracing_stack(
        make_writer,
        worker_guard,
        log_path,
    ))
}

/// Builds a file fmt stack with a custom wrapped writer (integration tests).
#[cfg(test)]
pub(crate) fn build_release_file_tracing_stack_with_make_writer<W>(
    app_data: &Path,
    run_session_id: &str,
    make_writer: ReleaseLogMakeWriter<W>,
) -> Result<
    ReleaseFileTracingStack<impl tracing::Subscriber + Send + Sync + SubscriberInitExt + 'static>,
    ReleaseLogInitError,
>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let (session_dir, log_path) =
        super::persistence::create_session_log_files(app_data, run_session_id)?;
    let (_non_blocking, worker_guard) =
        super::persistence::build_nonblocking_appender(&session_dir)?;
    Ok(assemble_release_file_tracing_stack(
        make_writer,
        worker_guard,
        log_path,
    ))
}

/// Creates session files, installs the release file fmt layer, and returns the worker guard.
pub fn attach_release_file_layer(
    app_data: &Path,
    config: &ReleaseLogConfig,
    run_session_id: &str,
) -> Result<LogGuardState, ReleaseLogInitError> {
    if !persistence_side_effects_allowed(config) {
        return Err(ReleaseLogInitError::new(
            "release file logging is not active for this build/config",
        ));
    }

    Ok(build_release_file_tracing_stack(app_data, run_session_id)?.install_global())
}

/// Attaches release file logging or degrades on failure, ignoring build-profile gating.
pub fn try_setup_release_file_logging_unchecked(
    app_data: &Path,
    run_session_id: &str,
) -> Option<LogGuardState> {
    match build_release_file_tracing_stack(app_data, run_session_id) {
        Ok(stack) => Some(stack.install_global()),
        Err(err) => degrade_after_persistence_failure(&err),
    }
}

/// Tauri setup hook: attach release file logging or degrade on failure.
pub fn setup_release_file_logging(
    app_data: &Path,
    config: &ReleaseLogConfig,
    run_session_id: &str,
) -> Option<LogGuardState> {
    if !persistence_side_effects_allowed(config) {
        return None;
    }

    try_setup_release_file_logging_unchecked(app_data, run_session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_env_filter_matches_contract() {
        assert_eq!(
            DEFAULT_ENV_FILTER,
            "gijirec_capture=info,gijirec_transcribe=info,gijirec_editor=info,info"
        );
        if std::env::var("RUST_LOG").is_err() {
            let expected = EnvFilter::new(DEFAULT_ENV_FILTER);
            assert_eq!(default_env_filter().to_string(), expected.to_string());
        }
    }

    #[test]
    fn debug_build_uses_stdout_mode_and_ignores_log_flag() {
        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        if cfg!(debug_assertions) {
            assert_eq!(
                select_subscriber_mode(&config),
                TracingSubscriberMode::DebugStdout
            );
            assert!(!persistence_side_effects_allowed(&config));
            assert!(!defers_subscriber_init_to_setup(&config));
        }
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_without_log_uses_noop_mode() {
        let config = ReleaseLogConfig {
            file_logging_enabled: false,
        };
        assert_eq!(
            select_subscriber_mode(&config),
            TracingSubscriberMode::ReleaseNoop
        );
        assert!(!persistence_side_effects_allowed(&config));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn release_with_log_defers_subscriber_init_to_setup() {
        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        assert_eq!(
            select_subscriber_mode(&config),
            TracingSubscriberMode::ReleaseFileLogging
        );
        assert!(persistence_side_effects_allowed(&config));
        assert!(defers_subscriber_init_to_setup(&config));
    }

    #[test]
    fn debug_init_does_not_create_logs_directory() {
        if !cfg!(debug_assertions) {
            return;
        }

        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        let app_data = unique_temp_dir("tracing-init-debug");
        let run_id = "20260906T074500Z-capture-0";

        apply_persistence_if_allowed(&app_data, &config, run_id)
            .expect("debug persistence hook must not fail");

        assert!(
            !app_data.join("logs").exists(),
            "debug init with --log must not create logs directory"
        );
    }

    #[test]
    fn setup_release_file_logging_returns_none_when_persistence_not_allowed() {
        let config = ReleaseLogConfig {
            file_logging_enabled: true,
        };
        let app_data = unique_temp_dir("setup-skipped");
        let run_id = "20260906T074500Z-capture-0";

        if cfg!(debug_assertions) {
            assert!(setup_release_file_logging(&app_data, &config, run_id).is_none());
            assert!(
                !app_data.join("logs").exists(),
                "disabled path must not create logs directory"
            );
        }
    }

    #[test]
    fn degrade_after_persistence_failure_returns_none_without_panic() {
        let err = ReleaseLogInitError::new("simulated persistence failure");
        assert!(degrade_after_persistence_failure(&err).is_none());
    }

    #[test]
    fn surface_persistence_failure_emits_contract_warn_field() {
        let err = ReleaseLogInitError::new("simulated persistence failure");
        let metadata = persistence_failure_warn_metadata(&err);
        assert_eq!(metadata.target, "gijirec_release_log");
        assert!(metadata.release_log_persistence_failed);
        assert_eq!(metadata.error, "simulated persistence failure");
        surface_persistence_failure(&err);
    }

    #[test]
    fn prepare_release_file_layer_builds_single_session_log_file() {
        let app_data = unique_temp_dir("prepare-layer");
        let run_id = "20260906T074500Z-capture-0";
        let (mut writer, guard, log_path) =
            crate::logging::persistence::prepare_release_file_layer(&app_data, run_id)
                .expect("prepare release file layer");
        use std::io::Write;
        writer
            .write_all(b"release log line\n")
            .expect("write release log line");
        drop(guard);
        assert!(log_path.is_file());
        assert_eq!(
            log_path.file_name().and_then(|name| name.to_str()),
            Some("gijirec.log")
        );
    }

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "gijirec-tracing-init-{label}-{}",
            std::process::id()
        ))
    }
}
