//! Audio device identity and session selection per `docs/contracts/audio-device-selection.md`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Input or output device classification per contract `AudioDeviceKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioDeviceKind {
    Input,
    Output,
}

/// Non-empty session device identifier (cpal 0.16 `Device::name()`; no public `Device::id()`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioDeviceId(String);

/// Errors when constructing an [`AudioDeviceId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioDeviceIdError {
    Empty,
}

impl fmt::Display for AudioDeviceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "audio device id must not be empty"),
        }
    }
}

impl std::error::Error for AudioDeviceIdError {}

impl AudioDeviceId {
    /// Builds a validated device id, rejecting empty or whitespace-only values.
    pub fn new(value: String) -> Result<Self, AudioDeviceIdError> {
        if value.trim().is_empty() {
            return Err(AudioDeviceIdError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for AudioDeviceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AudioDeviceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Listed device metadata for UI display (requirement 1.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    id: AudioDeviceId,
    name: String,
    kind: AudioDeviceKind,
    is_default: bool,
}

impl AudioDeviceInfo {
    pub fn new(id: AudioDeviceId, name: String, kind: AudioDeviceKind, is_default: bool) -> Self {
        Self {
            id,
            name,
            kind,
            is_default,
        }
    }

    pub fn id(&self) -> &AudioDeviceId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> AudioDeviceKind {
        self.kind
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }
}

/// Listed input and output devices per `docs/contracts/audio-device-selection.md`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AudioDeviceList {
    pub inputs: Vec<AudioDeviceInfo>,
    pub outputs: Vec<AudioDeviceInfo>,
}

/// Session-scoped device selection. `None` fields resolve to OS defaults (requirements 2.5–2.6).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeviceSelection {
    microphone_id: Option<AudioDeviceId>,
    speaker_id: Option<AudioDeviceId>,
}

impl DeviceSelection {
    pub fn new(microphone_id: Option<AudioDeviceId>, speaker_id: Option<AudioDeviceId>) -> Self {
        Self {
            microphone_id,
            speaker_id,
        }
    }

    pub fn microphone_id(&self) -> Option<&AudioDeviceId> {
        self.microphone_id.as_ref()
    }

    pub fn speaker_id(&self) -> Option<&AudioDeviceId> {
        self.speaker_id.as_ref()
    }

    /// `true` when the microphone should resolve to the OS default device.
    pub fn resolves_microphone_to_os_default(&self) -> bool {
        self.microphone_id.is_none()
    }

    /// `true` when the speaker should resolve to the OS default output device.
    pub fn resolves_speaker_to_os_default(&self) -> bool {
        self.speaker_id.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::fixtures;
    use serde_json::json;

    #[test]
    fn accepts_non_empty_device_id() {
        let id = AudioDeviceId::new("cpal-device-1".to_string()).expect("valid id");
        assert_eq!(id.as_str(), "cpal-device-1");
    }

    #[test]
    fn rejects_empty_device_id() {
        let err = AudioDeviceId::new(String::new()).unwrap_err();
        assert_eq!(err, AudioDeviceIdError::Empty);
    }

    #[test]
    fn rejects_whitespace_only_device_id() {
        let err = AudioDeviceId::new("   \t\n".to_string()).unwrap_err();
        assert_eq!(err, AudioDeviceIdError::Empty);
    }

    #[test]
    fn device_selection_default_uses_os_defaults() {
        fixtures::assert_both_channels_resolve_to_os_default(&DeviceSelection::default());
    }

    #[test]
    fn device_selection_none_is_default_resolvable() {
        let selection = DeviceSelection::new(None, None);
        assert!(selection.resolves_microphone_to_os_default());
        assert!(selection.resolves_speaker_to_os_default());
    }

    #[test]
    fn device_selection_explicit_ids_do_not_resolve_to_os_default() {
        let mic = AudioDeviceId::new("mic-1".to_string()).expect("mic id");
        let speaker = AudioDeviceId::new("spk-1".to_string()).expect("speaker id");
        let selection = DeviceSelection::new(Some(mic), Some(speaker));

        assert!(!selection.resolves_microphone_to_os_default());
        assert!(!selection.resolves_speaker_to_os_default());
        assert_eq!(
            selection.microphone_id().map(|id| id.as_str()),
            Some("mic-1")
        );
        assert_eq!(selection.speaker_id().map(|id| id.as_str()), Some("spk-1"));
    }

    #[test]
    fn audio_device_info_exposes_display_name_and_kind() {
        let id = AudioDeviceId::new("dev-1".to_string()).expect("device id");
        let info = AudioDeviceInfo::new(
            id,
            "Built-in Microphone".to_string(),
            AudioDeviceKind::Input,
            true,
        );

        assert_eq!(info.id().as_str(), "dev-1");
        assert_eq!(info.name(), "Built-in Microphone");
        assert_eq!(info.kind(), AudioDeviceKind::Input);
        assert!(info.is_default());
    }

    #[test]
    fn audio_device_kind_serializes_to_contract_values() {
        assert_eq!(
            serde_json::to_value(AudioDeviceKind::Input).expect("serialize input"),
            json!("input")
        );
        assert_eq!(
            serde_json::to_value(AudioDeviceKind::Output).expect("serialize output"),
            json!("output")
        );
    }

    #[test]
    fn device_selection_serializes_null_for_os_default() {
        let selection = DeviceSelection::default();
        let value = serde_json::to_value(&selection).expect("serialize selection");

        assert_eq!(value["microphone_id"], json!(null));
        assert_eq!(value["speaker_id"], json!(null));
    }
}
