//! Host tracing backend for device selection observability.

use gijirec_presentation::application::device_selection::DeviceSelectionObservability;

/// Target for device selection tracing (`RUST_LOG=gijirec_device=debug`).
pub const DEVICE_SELECTION_LOG_TARGET: &str = "gijirec_device";

/// Emits structured device-selection events via `tracing` (host-only; no PCM or device names at INFO).
pub struct TracingDeviceSelectionObservability;

impl DeviceSelectionObservability for TracingDeviceSelectionObservability {
    fn log_selection_changed(&self, microphone_id: Option<&str>, speaker_id: Option<&str>) {
        tracing::info!(
            target: DEVICE_SELECTION_LOG_TARGET,
            microphone_id = microphone_id.unwrap_or("null"),
            speaker_id = speaker_id.unwrap_or("null"),
            "device selection changed"
        );
    }

    fn log_recapture_started(
        &self,
        correlation_id: &str,
        microphone_id: Option<&str>,
        speaker_id: Option<&str>,
    ) {
        tracing::info!(
            target: DEVICE_SELECTION_LOG_TARGET,
            correlation_id,
            microphone_id = microphone_id.unwrap_or("null"),
            speaker_id = speaker_id.unwrap_or("null"),
            "device selection recapture started"
        );
    }

    fn log_recapture_completed(&self, correlation_id: &str, duration_ms: u64) {
        tracing::info!(
            target: DEVICE_SELECTION_LOG_TARGET,
            correlation_id,
            device_selection_restart_duration_ms = duration_ms,
            "device selection recapture completed"
        );
    }

    fn log_device_names_debug(&self, microphone_name: Option<&str>, speaker_name: Option<&str>) {
        tracing::debug!(
            target: DEVICE_SELECTION_LOG_TARGET,
            microphone_name = microphone_name.unwrap_or("null"),
            speaker_name = speaker_name.unwrap_or("null"),
            "device selection display names"
        );
    }
}
