//! Thread-safe transcribe phase/progress snapshot for IPC without blocking on orchestrator locks.

use std::sync::Mutex;

use gijirec_application::transcribe::ModelDownloadProgress;
use gijirec_domain::transcribe::TranscribePhase;
use serde::Serialize;

use super::event_emitter::{TranscribeModelProgressPayload, TranscribePhaseChangedPayload};

/// Cached transcribe status returned to the frontend on mount.
#[derive(Debug, Clone, Serialize)]
pub struct TranscribeStatusSnapshot {
    pub phase: TranscribePhaseChangedPayload,
    pub model_progress: Option<TranscribeModelProgressPayload>,
}

/// Latest transcribe status readable from the main thread while model acquisition runs in background.
#[derive(Debug)]
pub struct TranscribeStatusCache {
    phase: Mutex<TranscribePhaseChangedPayload>,
    progress: Mutex<Option<TranscribeModelProgressPayload>>,
}

impl Default for TranscribeStatusCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscribeStatusCache {
    pub fn new() -> Self {
        Self {
            phase: Mutex::new(TranscribePhaseChangedPayload {
                phase: TranscribePhase::Idle.as_str().to_string(),
                timestamp_ms: 0,
            }),
            progress: Mutex::new(None),
        }
    }

    pub fn set_phase(&self, phase: TranscribePhase, timestamp_ms: u64) {
        if let Ok(mut guard) = self.phase.lock() {
            *guard = TranscribePhaseChangedPayload {
                phase: phase.as_str().to_string(),
                timestamp_ms,
            };
        }
    }

    pub fn set_progress(&self, progress: &ModelDownloadProgress) {
        if let Ok(mut guard) = self.progress.lock() {
            *guard = Some(TranscribeModelProgressPayload::from(progress));
        }
    }

    pub fn clear_progress(&self) {
        if let Ok(mut guard) = self.progress.lock() {
            *guard = None;
        }
    }

    pub fn phase_payload(&self) -> TranscribePhaseChangedPayload {
        self.phase
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| TranscribePhaseChangedPayload {
                phase: TranscribePhase::Idle.as_str().to_string(),
                timestamp_ms: 0,
            })
    }

    pub fn progress_payload(&self) -> Option<TranscribeModelProgressPayload> {
        self.progress.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn snapshot(&self) -> TranscribeStatusSnapshot {
        TranscribeStatusSnapshot {
            phase: self.phase_payload(),
            model_progress: self.progress_payload(),
        }
    }
}
