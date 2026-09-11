//! Windows WASAPI loopback adapter using cpal on the default or selected output device.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BuildStreamError, Device, Host, Stream};
use gijirec_domain::audio::{AudioDeviceId, CaptureError};

use crate::audio::mic_capture::StreamRuntimeErrorCallback;

/// Loopback capture adapter streaming f32 mono samples into an rtrb consumer.
pub struct WindowsLoopbackAdapter {
    #[expect(dead_code)]
    stream: Stream,
}

/// Consumer side of the loopback ring buffer.
pub type LoopbackSampleConsumer = crate::audio::f32_ring_consumer::F32RingConsumer;

impl WindowsLoopbackAdapter {
    /// Opens the default output device for WASAPI loopback capture.
    pub fn open(ring_capacity: usize) -> Result<(Self, LoopbackSampleConsumer, u32), CaptureError> {
        Self::open_with_device_id(None, ring_capacity)
    }

    /// Opens loopback on the default output (`None`) or a named output device.
    pub fn open_with_device_id(
        device_id: Option<&AudioDeviceId>,
        ring_capacity: usize,
    ) -> Result<(Self, LoopbackSampleConsumer, u32), CaptureError> {
        Self::open_with_device_id_and_runtime_hook(device_id, ring_capacity, None)
    }

    /// Opens loopback with a runtime error callback (req 4.3 stream disconnect).
    pub fn open_with_device_id_and_runtime_hook(
        device_id: Option<&AudioDeviceId>,
        ring_capacity: usize,
        on_stream_error: Option<StreamRuntimeErrorCallback>,
    ) -> Result<(Self, LoopbackSampleConsumer, u32), CaptureError> {
        let host = cpal::default_host();
        let (device, selected) = match device_id {
            None => (
                host.default_output_device()
                    .ok_or(CaptureError::SystemAudioUnavailable)?,
                false,
            ),
            Some(id) => (find_output_device(&host, id)?, true),
        };
        Self::open_device_inner(&device, ring_capacity, selected, on_stream_error)
    }

    /// Opens a specific output device (used in tests and composition root).
    pub fn open_device(
        device: &Device,
        ring_capacity: usize,
    ) -> Result<(Self, LoopbackSampleConsumer, u32), CaptureError> {
        Self::open_device_inner(device, ring_capacity, true, None)
    }

    fn open_device_inner(
        device: &Device,
        ring_capacity: usize,
        selected: bool,
        on_stream_error: Option<StreamRuntimeErrorCallback>,
    ) -> Result<(Self, LoopbackSampleConsumer, u32), CaptureError> {
        let supported = device
            .default_output_config()
            .map_err(|err| map_output_config_error(err, selected))?;
        let (stream, consumer, sample_rate_hz) =
            crate::audio::cpal_mono_input::build_and_play_mono_input_stream(
                device,
                &supported,
                ring_capacity,
                "system",
                selected,
                on_stream_error,
                map_build_error,
                map_play_error,
                |other| format!("unsupported loopback sample format: {other:?}"),
            )?;

        Ok((
            Self { stream },
            LoopbackSampleConsumer::from_ring_consumer(consumer),
            sample_rate_hz,
        ))
    }
}

/// Resolves an output device by cpal `Device::name()` (session-stable `AudioDeviceId`).
pub(crate) fn find_output_device(
    host: &Host,
    device_id: &AudioDeviceId,
) -> Result<Device, CaptureError> {
    crate::audio::cpal_mono_input::find_device_matching_id(
        host.output_devices()
            .map_err(|err| CaptureError::Internal {
                detail: err.to_string(),
            })?,
        device_id,
        |device| device.name(),
        |err| CaptureError::Internal {
            detail: err.to_string(),
        },
        CaptureError::SelectedSystemAudioUnavailable,
    )
}

fn map_output_config_error(err: cpal::DefaultStreamConfigError, selected: bool) -> CaptureError {
    if selected {
        CaptureError::SelectedSystemAudioUnavailable
    } else {
        CaptureError::Internal {
            detail: format!("failed to query default output config: {err}"),
        }
    }
}

fn map_build_error(err: BuildStreamError, selected: bool) -> CaptureError {
    crate::audio::cpal_mono_input::map_stream_error(
        crate::audio::cpal_mono_input::CpalStreamError::Build(err),
        selected,
        CaptureError::SelectedSystemAudioUnavailable,
        CaptureError::SystemAudioUnavailable,
        |err| CaptureError::Internal {
            detail: err.to_string(),
        },
    )
}

fn map_play_error(err: cpal::PlayStreamError, selected: bool) -> CaptureError {
    crate::audio::cpal_mono_input::map_stream_error(
        crate::audio::cpal_mono_input::CpalStreamError::Play(err),
        selected,
        CaptureError::SelectedSystemAudioUnavailable,
        CaptureError::SystemAudioUnavailable,
        |err| CaptureError::Internal {
            detail: err.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mic_capture::DEFAULT_RING_CAPACITY;
    use gijirec_domain::audio::AudioDeviceId;

    /// Documented skip reason for Windows WASAPI loopback hardware tests (Integration Test 1).
    /// Must match `#[ignore = "..."]` on `opens_default_loopback_on_hardware` exactly.
    pub(crate) const WINDOWS_LOOPBACK_HARDWARE_SKIP: &str = "CI: requires Windows default WASAPI loopback output device; run with --ignored on local hardware";

    /// Documented skip reason for non-default output loopback hardware test (design 6.2).
    pub(crate) const WINDOWS_LOOPBACK_NON_DEFAULT_HARDWARE_SKIP: &str = "CI: requires Windows with multiple WASAPI output devices; run with --ignored on local hardware";

    #[test]
    fn documents_wasapi_loopback_hardware_ci_skip_reason() {
        let reason = WINDOWS_LOOPBACK_HARDWARE_SKIP;
        assert_eq!(
            reason,
            "CI: requires Windows default WASAPI loopback output device; run with --ignored on local hardware"
        );
        assert!(reason.contains("WASAPI"));
        assert!(reason.contains("loopback"));
        assert!(reason.contains("CI"));
    }

    #[test]
    fn documents_non_default_loopback_hardware_ci_skip_reason() {
        let reason = WINDOWS_LOOPBACK_NON_DEFAULT_HARDWARE_SKIP;
        assert_eq!(
            reason,
            "CI: requires Windows with multiple WASAPI output devices; run with --ignored on local hardware"
        );
        assert!(reason.contains("WASAPI"));
        assert!(reason.contains("multiple"));
    }

    #[test]
    fn find_output_device_returns_selected_system_audio_unavailable_for_unknown_id() {
        crate::audio::cpal_device_test_support::assert_find_device_returns_error_for_unknown_id(
            find_output_device,
            CaptureError::SelectedSystemAudioUnavailable,
        );
    }

    #[test]
    fn open_with_device_id_returns_selected_system_audio_unavailable_for_unknown_id() {
        let device_id = crate::audio::cpal_device_test_support::unknown_device_id();

        let err =
            WindowsLoopbackAdapter::open_with_device_id(Some(&device_id), DEFAULT_RING_CAPACITY)
                .err()
                .expect("error");
        assert_eq!(err, CaptureError::SelectedSystemAudioUnavailable);
        assert_eq!(
            err.to_user_facing().code.as_str(),
            "SELECTED_SYSTEM_AUDIO_UNAVAILABLE"
        );
    }

    #[test]
    fn find_output_device_resolves_device_when_name_matches() {
        use cpal::traits::HostTrait;

        crate::audio::cpal_device_test_support::assert_resolves_device_when_default_name_matches(
            |host| host.default_output_device(),
            find_output_device,
        );
    }

    // Integration Test 1 (WindowsLoopbackAdapter): default WASAPI loopback opens on hardware
    #[test]
    #[ignore = "CI: requires Windows default WASAPI loopback output device; run with --ignored on local hardware"]
    fn opens_default_loopback_on_hardware() {
        let (_adapter, consumer, sample_rate_hz) =
            WindowsLoopbackAdapter::open(DEFAULT_RING_CAPACITY).expect("default loopback");
        crate::audio::cpal_device_test_support::assert_cpal_consumer_receives_samples(
            sample_rate_hz,
            consumer,
            200,
        );
    }

    // Integration Test 5 (design 6.2): non-default output device loopback on hardware
    #[test]
    #[ignore = "CI: requires Windows with multiple WASAPI output devices; run with --ignored on local hardware"]
    fn opens_non_default_output_loopback_on_hardware() {
        let host = cpal::default_host();
        let default_name = host
            .default_output_device()
            .expect("default output")
            .name()
            .expect("default output name");

        let non_default = host
            .output_devices()
            .expect("output devices")
            .find(|device| {
                device
                    .name()
                    .map(|name| name != default_name)
                    .unwrap_or(false)
            })
            .expect("non-default output device");

        let device_id =
            AudioDeviceId::new(non_default.name().expect("device name")).expect("valid id");

        let (_adapter, consumer, sample_rate_hz) =
            WindowsLoopbackAdapter::open_with_device_id(Some(&device_id), DEFAULT_RING_CAPACITY)
                .expect("non-default loopback");
        crate::audio::cpal_device_test_support::assert_cpal_consumer_receives_samples(
            sample_rate_hz,
            consumer,
            200,
        );
    }
}
