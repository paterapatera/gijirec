//! Tauri IPC commands for capture and transcribe status.

use gijirec_presentation::tauri::events::CapturePhaseChangedPayload;
use gijirec_presentation::tauri::lifecycle::CaptureLifecycleState;
use gijirec_presentation::transcribe::event_emitter::TranscribePhaseChangedPayload;
use gijirec_presentation::transcribe::{TranscribeStatusCache, TranscribeStatusSnapshot};
use std::sync::Arc;
use tauri::State;

/// Returns the orchestrator's current phase (sync on frontend mount).
#[tauri::command]
pub fn get_capture_phase(
    state: State<'_, CaptureLifecycleState>,
) -> Result<CapturePhaseChangedPayload, String> {
    state.current_phase_payload()
}

/// Returns the cached transcribe phase (non-blocking during model download).
#[tauri::command]
pub fn get_transcribe_phase(
    cache: State<'_, Arc<TranscribeStatusCache>>,
) -> Result<TranscribePhaseChangedPayload, String> {
    Ok(cache.phase_payload())
}

/// Returns cached transcribe status for frontend mount sync.
#[tauri::command]
pub fn get_transcribe_status(
    cache: State<'_, Arc<TranscribeStatusCache>>,
) -> Result<TranscribeStatusSnapshot, String> {
    Ok(cache.snapshot())
}
