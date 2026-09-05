//! Capture observability dispatch (no tracing macros — bylaw-safe in presentation).

use gijirec_domain::audio::{CaptureError, CapturePhase};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// Structured capture observability hooks (implemented by the host with `tracing`).
pub trait CaptureObservability: Send + Sync {
    fn log_phase_transition(&self, phase: CapturePhase);
    fn log_buffer_drop(&self, drops_total: u64);
    fn log_stream_open_failure(&self, port: &str, error: &CaptureError, correlation_id: &str);
    fn log_rt_callback_max_us(&self, max_us: u64);
}

struct NoopObservability;

impl CaptureObservability for NoopObservability {
    fn log_phase_transition(&self, _phase: CapturePhase) {}
    fn log_buffer_drop(&self, _drops_total: u64) {}
    fn log_stream_open_failure(&self, _port: &str, _error: &CaptureError, _correlation_id: &str) {}
    fn log_rt_callback_max_us(&self, _max_us: u64) {}
}

static OBSERVABILITY: OnceLock<RwLock<Box<dyn CaptureObservability>>> = OnceLock::new();

fn observability() -> &'static RwLock<Box<dyn CaptureObservability>> {
    OBSERVABILITY.get_or_init(|| RwLock::new(Box::new(NoopObservability)))
}

/// Registers or replaces the observability backend (call from `run()` and tests).
pub fn set_observability(backend: Box<dyn CaptureObservability>) {
    if let Some(lock) = OBSERVABILITY.get() {
        *lock.write().expect("lock") = backend;
    } else {
        let _ = OBSERVABILITY.set(RwLock::new(backend));
    }
}

/// Target name for host tracing (`RUST_LOG=gijirec_capture=debug`).
pub const CAPTURE_LOG_TARGET: &str = "gijirec_capture";

static SESSION_ID: OnceLock<String> = OnceLock::new();
static SESSION_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns a stable correlation id for the current capture session.
pub fn session_id() -> &'static str {
    SESSION_ID.get_or_init(|| format!("capture-{}", SESSION_SEQ.fetch_add(1, Ordering::Relaxed)))
}

/// Initializes the capture session correlation id (call once at app startup).
pub fn init_session_id() {
    let _ = session_id();
}

/// Logs a capture lifecycle phase transition at INFO with `capture_phase` gauge field.
pub fn log_phase_transition(phase: CapturePhase) {
    observability()
        .read()
        .expect("lock")
        .log_phase_transition(phase);
}

/// Logs a PCM bus buffer drop at WARN with `capture_buffer_drops_total` counter field.
pub fn log_buffer_drop(drops_total: u64) {
    observability()
        .read()
        .expect("lock")
        .log_buffer_drop(drops_total);
}

/// Logs a stream open failure at ERROR with `error_code`; never logs device names or PCM.
pub fn log_stream_open_failure(port: &str, error: &CaptureError, correlation_id: &str) {
    observability()
        .read()
        .expect("lock")
        .log_stream_open_failure(port, error, correlation_id);
}

/// Logs the observed RT callback or drain latency maximum in microseconds.
pub fn log_rt_callback_max_us(max_us: u64) {
    observability()
        .read()
        .expect("lock")
        .log_rt_callback_max_us(max_us);
}

/// Records observability events for unit tests.
#[derive(Clone)]
pub struct RecordingObservability {
    pub phases: Arc<std::sync::Mutex<Vec<CapturePhase>>>,
    pub drops: Arc<std::sync::Mutex<Vec<u64>>>,
    pub stream_failures: Arc<std::sync::Mutex<Vec<(String, CaptureError)>>>,
}

impl RecordingObservability {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for RecordingObservability {
    fn default() -> Self {
        Self {
            phases: Arc::new(std::sync::Mutex::new(Vec::new())),
            drops: Arc::new(std::sync::Mutex::new(Vec::new())),
            stream_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl CaptureObservability for RecordingObservability {
    fn log_phase_transition(&self, phase: CapturePhase) {
        self.phases.lock().expect("lock").push(phase);
    }

    fn log_buffer_drop(&self, drops_total: u64) {
        self.drops.lock().expect("lock").push(drops_total);
    }

    fn log_stream_open_failure(&self, port: &str, error: &CaptureError, _correlation_id: &str) {
        self.stream_failures
            .lock()
            .expect("lock")
            .push((port.to_string(), error.clone()));
    }

    fn log_rt_callback_max_us(&self, _max_us: u64) {}
}
