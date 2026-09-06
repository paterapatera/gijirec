//! Device selection observability hooks (no tracing in application layer).

use std::sync::{Arc, Mutex};

/// Structured device-selection observability (host implements with `tracing`).
pub trait DeviceSelectionObservability: Send + Sync {
    /// INFO: selection applied (device IDs only; `None` = OS default).
    fn log_selection_changed(&self, microphone_id: Option<&str>, speaker_id: Option<&str>);

    /// INFO: recapture restart begins for a selection change session.
    fn log_recapture_started(
        &self,
        correlation_id: &str,
        microphone_id: Option<&str>,
        speaker_id: Option<&str>,
    );

    /// INFO: recapture restart finished; `duration_ms` maps to `device_selection_restart_duration_ms`.
    fn log_recapture_completed(&self, correlation_id: &str, duration_ms: u64);

    /// DEBUG: human-readable device names (never INFO).
    fn log_device_names_debug(&self, microphone_name: Option<&str>, speaker_name: Option<&str>);
}

/// No-op backend for tests and default wiring.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopDeviceSelectionObservability;

impl DeviceSelectionObservability for NoopDeviceSelectionObservability {
    fn log_selection_changed(&self, _microphone_id: Option<&str>, _speaker_id: Option<&str>) {}

    fn log_recapture_started(
        &self,
        _correlation_id: &str,
        _microphone_id: Option<&str>,
        _speaker_id: Option<&str>,
    ) {
    }

    fn log_recapture_completed(&self, _correlation_id: &str, _duration_ms: u64) {}

    fn log_device_names_debug(&self, _microphone_name: Option<&str>, _speaker_name: Option<&str>) {}
}

/// In-memory recorder for unit tests.
#[derive(Clone, Default)]
#[allow(clippy::type_complexity)]
pub struct RecordingDeviceSelectionObservability {
    pub selection_changed: Arc<Mutex<Vec<(Option<String>, Option<String>)>>>,
    pub recapture_started: Arc<Mutex<Vec<(String, Option<String>, Option<String>)>>>,
    pub recapture_completed: Arc<Mutex<Vec<(String, u64)>>>,
    pub device_names_debug: Arc<Mutex<Vec<(Option<String>, Option<String>)>>>,
}

impl RecordingDeviceSelectionObservability {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DeviceSelectionObservability for RecordingDeviceSelectionObservability {
    fn log_selection_changed(&self, microphone_id: Option<&str>, speaker_id: Option<&str>) {
        self.selection_changed.lock().expect("lock").push((
            microphone_id.map(str::to_string),
            speaker_id.map(str::to_string),
        ));
    }

    fn log_recapture_started(
        &self,
        correlation_id: &str,
        microphone_id: Option<&str>,
        speaker_id: Option<&str>,
    ) {
        self.recapture_started.lock().expect("lock").push((
            correlation_id.to_string(),
            microphone_id.map(str::to_string),
            speaker_id.map(str::to_string),
        ));
    }

    fn log_recapture_completed(&self, correlation_id: &str, duration_ms: u64) {
        self.recapture_completed
            .lock()
            .expect("lock")
            .push((correlation_id.to_string(), duration_ms));
    }

    fn log_device_names_debug(&self, microphone_name: Option<&str>, speaker_name: Option<&str>) {
        self.device_names_debug.lock().expect("lock").push((
            microphone_name.map(str::to_string),
            speaker_name.map(str::to_string),
        ));
    }
}
