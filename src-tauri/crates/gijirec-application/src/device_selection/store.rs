//! Session-scoped device selection state (non-persistent).

use gijirec_domain::audio::DeviceSelection;
use std::sync::Mutex;

/// Thread-safe in-memory store for the current session's microphone and speaker selection.
///
/// `None` fields in [`DeviceSelection`] mean the OS default device should be used downstream.
pub struct DeviceSelectionStore {
    selection: Mutex<DeviceSelection>,
}

impl DeviceSelectionStore {
    pub fn new() -> Self {
        Self {
            selection: Mutex::new(DeviceSelection::default()),
        }
    }

    pub fn get_selection(&self) -> DeviceSelection {
        self.selection
            .lock()
            .expect("device selection lock poisoned")
            .clone()
    }

    pub fn update(&self, selection: DeviceSelection) {
        *self
            .selection
            .lock()
            .expect("device selection lock poisoned") = selection;
    }
}

impl Default for DeviceSelectionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceSelectionStore;
    use gijirec_domain::audio::{AudioDeviceId, DeviceSelection};

    /// Design unit test 8: initial store selection uses `None` → OS default resolution.
    #[test]
    fn none_selection_is_passed_as_default_resolvable() {
        let store = DeviceSelectionStore::new();
        let selection = store.get_selection();

        assert!(selection.microphone_id().is_none());
        assert!(selection.speaker_id().is_none());
        assert!(selection.resolves_microphone_to_os_default());
        assert!(selection.resolves_speaker_to_os_default());
    }

    /// Design unit test 8 (per-field): stored `None` on one channel only resolves that channel to OS default.
    #[test]
    fn store_passes_none_fields_as_os_default_resolution() {
        let store = DeviceSelectionStore::new();
        let speaker = AudioDeviceId::new("spk-1".to_string()).expect("speaker id");
        store.update(DeviceSelection::new(None, Some(speaker.clone())));

        let mic_default_speaker_explicit = store.get_selection();
        assert!(mic_default_speaker_explicit.microphone_id().is_none());
        assert!(mic_default_speaker_explicit.resolves_microphone_to_os_default());
        assert_eq!(
            mic_default_speaker_explicit
                .speaker_id()
                .map(|id| id.as_str()),
            Some("spk-1")
        );
        assert!(!mic_default_speaker_explicit.resolves_speaker_to_os_default());

        let microphone = AudioDeviceId::new("mic-1".to_string()).expect("mic id");
        store.update(DeviceSelection::new(Some(microphone.clone()), None));

        let mic_explicit_speaker_default = store.get_selection();
        assert_eq!(
            mic_explicit_speaker_default
                .microphone_id()
                .map(|id| id.as_str()),
            Some("mic-1")
        );
        assert!(!mic_explicit_speaker_default.resolves_microphone_to_os_default());
        assert!(mic_explicit_speaker_default.speaker_id().is_none());
        assert!(mic_explicit_speaker_default.resolves_speaker_to_os_default());
    }

    #[test]
    fn update_replaces_selection() {
        let store = DeviceSelectionStore::new();
        let mic = AudioDeviceId::new("mic-1".to_string()).expect("mic id");
        let speaker = AudioDeviceId::new("spk-1".to_string()).expect("speaker id");
        let explicit = DeviceSelection::new(Some(mic), Some(speaker));

        store.update(explicit.clone());

        let selection = store.get_selection();
        assert_eq!(selection, explicit);
        assert!(!selection.resolves_microphone_to_os_default());
        assert!(!selection.resolves_speaker_to_os_default());
    }

    #[test]
    fn update_with_none_restores_os_default_resolution() {
        let store = DeviceSelectionStore::new();
        let mic = AudioDeviceId::new("mic-1".to_string()).expect("mic id");
        store.update(DeviceSelection::new(Some(mic), None));
        store.update(DeviceSelection::default());

        let selection = store.get_selection();
        assert!(selection.resolves_microphone_to_os_default());
        assert!(selection.resolves_speaker_to_os_default());
    }
}
