//! cpal-backed audio input/output enumeration for device selection.

use std::fmt;

use cpal::traits::{DeviceTrait, HostTrait};
use gijirec_domain::audio::{AudioDeviceId, AudioDeviceInfo, AudioDeviceKind, AudioDeviceList};

/// Errors while enumerating audio devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumeratorError {
    Internal { detail: String },
}

impl fmt::Display for EnumeratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal { detail } => write!(f, "audio device enumeration failed: {detail}"),
        }
    }
}

impl std::error::Error for EnumeratorError {}

/// Host abstraction for testable device enumeration.
pub trait AudioDeviceHost {
    type Device: AudioDeviceSource;

    fn default_input(&self) -> Option<Self::Device>;
    fn default_output(&self) -> Option<Self::Device>;
    fn input_devices(&self) -> Result<Vec<Self::Device>, EnumeratorError>;
    fn output_devices(&self) -> Result<Vec<Self::Device>, EnumeratorError>;
}

/// Device abstraction exposing stable id and display name.
pub trait AudioDeviceSource {
    fn stable_id(&self) -> Result<String, EnumeratorError>;
    fn display_name(&self) -> Result<String, EnumeratorError>;
}

/// Enumerates cpal input/output devices with OS default flags.
pub struct AudioDeviceEnumerator<H = CpalAudioHost> {
    host: H,
}

impl AudioDeviceEnumerator<CpalAudioHost> {
    pub fn new() -> Self {
        Self {
            host: CpalAudioHost::default(),
        }
    }
}

impl Default for AudioDeviceEnumerator<CpalAudioHost> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: AudioDeviceHost> AudioDeviceEnumerator<H> {
    pub fn with_host(host: H) -> Self {
        Self { host }
    }

    pub fn list_devices(&self) -> Result<AudioDeviceList, EnumeratorError> {
        let default_input_id = self
            .host
            .default_input()
            .and_then(|device| device.stable_id().ok());
        let default_output_id = self
            .host
            .default_output()
            .and_then(|device| device.stable_id().ok());

        let inputs = collect_devices(
            self.host.input_devices()?,
            AudioDeviceKind::Input,
            default_input_id.as_deref(),
        )?;
        let outputs = collect_devices(
            self.host.output_devices()?,
            AudioDeviceKind::Output,
            default_output_id.as_deref(),
        )?;

        Ok(AudioDeviceList { inputs, outputs })
    }
}

fn collect_devices<D: AudioDeviceSource>(
    devices: Vec<D>,
    kind: AudioDeviceKind,
    default_id: Option<&str>,
) -> Result<Vec<AudioDeviceInfo>, EnumeratorError> {
    devices
        .into_iter()
        .map(|device| map_device_info(device, kind, default_id))
        .collect()
}

fn map_device_info<D: AudioDeviceSource>(
    device: D,
    kind: AudioDeviceKind,
    default_id: Option<&str>,
) -> Result<AudioDeviceInfo, EnumeratorError> {
    let id_value = device.stable_id()?;
    let name = device.display_name()?;
    let id = AudioDeviceId::new(id_value).map_err(|_| EnumeratorError::Internal {
        detail: "device id must not be empty".to_string(),
    })?;
    let is_default = default_id.is_some_and(|default| default == id.as_str());
    Ok(AudioDeviceInfo::new(id, name, kind, is_default))
}

/// cpal host wrapper for production enumeration.
pub struct CpalAudioHost(cpal::Host);

impl Default for CpalAudioHost {
    fn default() -> Self {
        Self(cpal::default_host())
    }
}

/// cpal device wrapper used by [`CpalAudioHost`].
#[derive(Clone)]
pub struct CpalDevice(cpal::Device);

impl AudioDeviceHost for CpalAudioHost {
    type Device = CpalDevice;

    fn default_input(&self) -> Option<Self::Device> {
        self.0.default_input_device().map(CpalDevice)
    }

    fn default_output(&self) -> Option<Self::Device> {
        self.0.default_output_device().map(CpalDevice)
    }

    fn input_devices(&self) -> Result<Vec<Self::Device>, EnumeratorError> {
        Ok(self
            .0
            .input_devices()
            .map_err(map_devices_error)?
            .map(CpalDevice)
            .collect())
    }

    fn output_devices(&self) -> Result<Vec<Self::Device>, EnumeratorError> {
        Ok(self
            .0
            .output_devices()
            .map_err(map_devices_error)?
            .map(CpalDevice)
            .collect())
    }
}

impl AudioDeviceSource for CpalDevice {
    fn stable_id(&self) -> Result<String, EnumeratorError> {
        cpal_device_stable_id(&self.0)
    }

    fn display_name(&self) -> Result<String, EnumeratorError> {
        self.0.name().map_err(map_name_error)
    }
}

/// cpal 0.16 does not expose `Device::id()`; use the OS device name as the session-stable identifier.
fn cpal_device_stable_id(device: &cpal::Device) -> Result<String, EnumeratorError> {
    device.name().map_err(map_name_error)
}

fn map_devices_error(err: cpal::DevicesError) -> EnumeratorError {
    EnumeratorError::Internal {
        detail: err.to_string(),
    }
}

fn map_name_error(err: cpal::DeviceNameError) -> EnumeratorError {
    EnumeratorError::Internal {
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::AudioDeviceKind;

    #[derive(Clone, Debug)]
    struct MockDevice {
        id: String,
        name: String,
    }

    impl AudioDeviceSource for MockDevice {
        fn stable_id(&self) -> Result<String, EnumeratorError> {
            Ok(self.id.clone())
        }

        fn display_name(&self) -> Result<String, EnumeratorError> {
            Ok(self.name.clone())
        }
    }

    struct MockHost {
        inputs: Vec<MockDevice>,
        outputs: Vec<MockDevice>,
        default_input_id: Option<String>,
        default_output_id: Option<String>,
    }

    impl AudioDeviceHost for MockHost {
        type Device = MockDevice;

        fn default_input(&self) -> Option<Self::Device> {
            self.default_input_id
                .as_ref()
                .and_then(|id| self.inputs.iter().find(|device| device.id == *id).cloned())
        }

        fn default_output(&self) -> Option<Self::Device> {
            self.default_output_id
                .as_ref()
                .and_then(|id| self.outputs.iter().find(|device| device.id == *id).cloned())
        }

        fn input_devices(&self) -> Result<Vec<Self::Device>, EnumeratorError> {
            Ok(self.inputs.clone())
        }

        fn output_devices(&self) -> Result<Vec<Self::Device>, EnumeratorError> {
            Ok(self.outputs.clone())
        }
    }

    fn mock_host() -> MockHost {
        MockHost {
            inputs: vec![
                MockDevice {
                    id: "mic-default".to_string(),
                    name: "Built-in Microphone".to_string(),
                },
                MockDevice {
                    id: "mic-usb".to_string(),
                    name: "USB Microphone".to_string(),
                },
            ],
            outputs: vec![
                MockDevice {
                    id: "spk-default".to_string(),
                    name: "Speakers (Realtek)".to_string(),
                },
                MockDevice {
                    id: "spk-hdmi".to_string(),
                    name: "HDMI Output".to_string(),
                },
            ],
            default_input_id: Some("mic-default".to_string()),
            default_output_id: Some("spk-default".to_string()),
        }
    }

    /// Design unit test 7: mock host maps stable id, display name, and `is_default` per device.
    #[test]
    fn mock_host_returns_display_names_and_is_default_flags() {
        let list = AudioDeviceEnumerator::with_host(mock_host())
            .list_devices()
            .expect("list devices");

        let mic_default = list
            .inputs
            .iter()
            .find(|device| device.id().as_str() == "mic-default")
            .expect("mic-default");
        assert_eq!(mic_default.name(), "Built-in Microphone");
        assert_eq!(mic_default.kind(), AudioDeviceKind::Input);
        assert!(mic_default.is_default());

        let mic_usb = list
            .inputs
            .iter()
            .find(|device| device.id().as_str() == "mic-usb")
            .expect("mic-usb");
        assert_eq!(mic_usb.name(), "USB Microphone");
        assert!(!mic_usb.is_default());

        let spk_default = list
            .outputs
            .iter()
            .find(|device| device.id().as_str() == "spk-default")
            .expect("spk-default");
        assert_eq!(spk_default.name(), "Speakers (Realtek)");
        assert_eq!(spk_default.kind(), AudioDeviceKind::Output);
        assert!(spk_default.is_default());

        let spk_hdmi = list
            .outputs
            .iter()
            .find(|device| device.id().as_str() == "spk-hdmi")
            .expect("spk-hdmi");
        assert_eq!(spk_hdmi.name(), "HDMI Output");
        assert!(!spk_hdmi.is_default());
    }

    #[test]
    fn lists_inputs_and_outputs_with_ids_and_display_names() {
        let list = AudioDeviceEnumerator::with_host(mock_host())
            .list_devices()
            .expect("list devices");

        assert_eq!(list.inputs.len(), 2);
        assert_eq!(list.outputs.len(), 2);

        assert_eq!(list.inputs[0].id().as_str(), "mic-default");
        assert_eq!(list.inputs[0].name(), "Built-in Microphone");
        assert_eq!(list.inputs[0].kind(), AudioDeviceKind::Input);

        assert_eq!(list.outputs[1].id().as_str(), "spk-hdmi");
        assert_eq!(list.outputs[1].name(), "HDMI Output");
        assert_eq!(list.outputs[1].kind(), AudioDeviceKind::Output);
    }

    #[test]
    fn marks_exactly_one_default_per_kind() {
        let list = AudioDeviceEnumerator::with_host(mock_host())
            .list_devices()
            .expect("list devices");

        let default_inputs = list.inputs.iter().filter(|d| d.is_default()).count();
        let default_outputs = list.outputs.iter().filter(|d| d.is_default()).count();

        assert_eq!(default_inputs, 1);
        assert_eq!(default_outputs, 1);
        assert!(
            list.inputs
                .iter()
                .any(|d| d.id().as_str() == "mic-default" && d.is_default())
        );
        assert!(
            list.outputs
                .iter()
                .any(|d| d.id().as_str() == "spk-default" && d.is_default())
        );
    }

    #[test]
    fn marks_no_defaults_when_os_default_missing() {
        let host = MockHost {
            default_input_id: None,
            default_output_id: None,
            ..mock_host()
        };

        let list = AudioDeviceEnumerator::with_host(host)
            .list_devices()
            .expect("list devices");

        assert_eq!(list.inputs.iter().filter(|d| d.is_default()).count(), 0);
        assert_eq!(list.outputs.iter().filter(|d| d.is_default()).count(), 0);
    }
}
