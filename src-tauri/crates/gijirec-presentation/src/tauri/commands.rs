//! Tauri IPC commands for capture status.

use crate::tauri::events::CapturePhaseChangedPayload;
use crate::tauri::lifecycle::CaptureLifecycleState;
use tauri::State;

/// Returns the orchestrator's current phase (sync on frontend mount).
#[tauri::command]
pub fn get_capture_phase(
    state: State<'_, CaptureLifecycleState>,
) -> Result<CapturePhaseChangedPayload, String> {
    state.current_phase_payload()
}
