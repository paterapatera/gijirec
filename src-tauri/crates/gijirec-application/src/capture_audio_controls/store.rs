//! Session-scoped capture audio controls state (non-persistent).

use gijirec_domain::audio::CaptureAudioControls;
use std::sync::Mutex;

/// Thread-safe in-memory store for session capture audio controls.
///
/// Values persist across device recapture; the store is not reset on capture restart.
pub struct CaptureAudioControlsStore {
    controls: Mutex<CaptureAudioControls>,
}

impl CaptureAudioControlsStore {
    pub fn new() -> Self {
        Self {
            controls: Mutex::new(CaptureAudioControls::default()),
        }
    }

    pub fn get_controls(&self) -> CaptureAudioControls {
        *self
            .controls
            .lock()
            .expect("capture audio controls lock poisoned")
    }

    pub fn update(&self, controls: CaptureAudioControls) {
        *self
            .controls
            .lock()
            .expect("capture audio controls lock poisoned") = controls;
    }
}

impl Default for CaptureAudioControlsStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureAudioControlsStore;
    use gijirec_domain::audio::{CaptureAudioControls, DEFAULT_INGEST_GAIN};

    #[test]
    fn new_store_uses_contract_defaults() {
        let store = CaptureAudioControlsStore::new();
        let controls = store.get_controls();

        assert!(controls.mic_ingest_enabled);
        assert_eq!(controls.manual_ingest_gain, DEFAULT_INGEST_GAIN);
        assert!(!controls.gain_user_adjusted);
    }

    #[test]
    fn update_replaces_controls() {
        let store = CaptureAudioControlsStore::new();
        let updated = CaptureAudioControls {
            mic_ingest_enabled: false,
            manual_ingest_gain: 2.0,
            gain_user_adjusted: true,
        };

        store.update(updated);
        assert_eq!(store.get_controls(), updated);
    }

    #[test]
    fn store_retains_values_after_simulated_recapture() {
        let store = CaptureAudioControlsStore::new();
        let custom = CaptureAudioControls {
            mic_ingest_enabled: false,
            manual_ingest_gain: 3.5,
            gain_user_adjusted: true,
        };
        store.update(custom);

        // Simulated device recapture does not touch this store.
        assert_eq!(store.get_controls(), custom);
    }
}
