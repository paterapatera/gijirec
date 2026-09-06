//! Windows WASAPI loopback adapter using cpal on the default or selected output device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BuildStreamError, Device, Host, SampleFormat, Stream, StreamConfig};
use gijirec_domain::audio::{AudioDeviceId, CaptureError};
use rtrb::RingBuffer;

use crate::audio::mic_capture::{
    StreamRuntimeErrorCallback, invoke_loopback_stream_runtime_error, push_mono_f32, push_mono_i16,
    push_mono_u16,
};

/// Loopback capture adapter streaming f32 mono samples into an rtrb consumer.
pub struct WindowsLoopbackAdapter {
    #[expect(dead_code)]
    stream: Stream,
}

/// Consumer side of the loopback ring buffer.
pub struct LoopbackSampleConsumer {
    inner: rtrb::Consumer<f32>,
}

impl LoopbackSampleConsumer {
    pub fn pop(&mut self) -> Option<f32> {
        self.inner.pop().ok()
    }

    /// Creates a consumer from an existing rtrb queue (synthetic streams in integration tests).
    pub fn from_ring_consumer(inner: rtrb::Consumer<f32>) -> Self {
        Self { inner }
    }

    pub fn drain_into(&mut self, out: &mut [f32]) -> usize {
        let mut count = 0;
        for slot in out.iter_mut() {
            match self.inner.pop() {
                Ok(sample) => {
                    *slot = sample;
                    count += 1;
                }
                Err(_) => break,
            }
        }
        count
    }

    pub fn slots(&self) -> usize {
        self.inner.slots()
    }
}

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
        let config: StreamConfig = supported.clone().into();
        let sample_rate_hz = config.sample_rate.0;
        let channels = supported.channels() as usize;
        let (producer, consumer) = RingBuffer::<f32>::new(ring_capacity);

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_f32_loopback(
                device,
                &config,
                channels,
                producer,
                selected,
                on_stream_error.clone(),
            )?,
            SampleFormat::I16 => build_i16_loopback(
                device,
                &config,
                channels,
                producer,
                selected,
                on_stream_error.clone(),
            )?,
            SampleFormat::U16 => build_u16_loopback(
                device,
                &config,
                channels,
                producer,
                selected,
                on_stream_error,
            )?,
            other => {
                return Err(CaptureError::Internal {
                    detail: format!("unsupported loopback sample format: {other:?}"),
                });
            }
        };

        stream.play().map_err(|err| map_play_error(err, selected))?;

        Ok((
            Self { stream },
            LoopbackSampleConsumer { inner: consumer },
            sample_rate_hz,
        ))
    }
}

/// Resolves an output device by cpal `Device::name()` (session-stable `AudioDeviceId`).
pub(crate) fn find_output_device(
    host: &Host,
    device_id: &AudioDeviceId,
) -> Result<Device, CaptureError> {
    let devices = host
        .output_devices()
        .map_err(|err| CaptureError::Internal {
            detail: err.to_string(),
        })?;

    for device in devices {
        let name = device.name().map_err(|err| CaptureError::Internal {
            detail: err.to_string(),
        })?;
        if name == device_id.as_str() {
            return Ok(device);
        }
    }

    Err(CaptureError::SelectedSystemAudioUnavailable)
}

#[allow(clippy::too_many_arguments)]
fn build_f32_loopback(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
    selected: bool,
    on_stream_error: Option<StreamRuntimeErrorCallback>,
) -> Result<Stream, CaptureError> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| push_mono_f32(data, channels, &mut producer),
            move |err| {
                invoke_loopback_stream_runtime_error("system", &err, on_stream_error.as_ref())
            },
            None,
        )
        .map_err(|err| map_build_error(err, selected))
}

#[allow(clippy::too_many_arguments)]
fn build_i16_loopback(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
    selected: bool,
    on_stream_error: Option<StreamRuntimeErrorCallback>,
) -> Result<Stream, CaptureError> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| push_mono_i16(data, channels, &mut producer),
            move |err| {
                invoke_loopback_stream_runtime_error("system", &err, on_stream_error.as_ref())
            },
            None,
        )
        .map_err(|err| map_build_error(err, selected))
}

#[allow(clippy::too_many_arguments)]
fn build_u16_loopback(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
    selected: bool,
    on_stream_error: Option<StreamRuntimeErrorCallback>,
) -> Result<Stream, CaptureError> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| push_mono_u16(data, channels, &mut producer),
            move |err| {
                invoke_loopback_stream_runtime_error("system", &err, on_stream_error.as_ref())
            },
            None,
        )
        .map_err(|err| map_build_error(err, selected))
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
    match err {
        BuildStreamError::DeviceNotAvailable | BuildStreamError::StreamConfigNotSupported => {
            if selected {
                CaptureError::SelectedSystemAudioUnavailable
            } else {
                CaptureError::SystemAudioUnavailable
            }
        }
        BuildStreamError::InvalidArgument => CaptureError::Internal {
            detail: err.to_string(),
        },
        BuildStreamError::BackendSpecific { err } => CaptureError::Internal {
            detail: err.to_string(),
        },
        other => CaptureError::Internal {
            detail: other.to_string(),
        },
    }
}

fn map_play_error(err: cpal::PlayStreamError, selected: bool) -> CaptureError {
    match err {
        cpal::PlayStreamError::DeviceNotAvailable => {
            if selected {
                CaptureError::SelectedSystemAudioUnavailable
            } else {
                CaptureError::SystemAudioUnavailable
            }
        }
        cpal::PlayStreamError::BackendSpecific { err } => CaptureError::Internal {
            detail: err.to_string(),
        },
    }
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
        let host = cpal::default_host();
        let device_id =
            AudioDeviceId::new("gijirec-nonexistent-output-id-xyz".to_string()).expect("valid id");

        assert!(matches!(
            find_output_device(&host, &device_id),
            Err(CaptureError::SelectedSystemAudioUnavailable)
        ));
    }

    #[test]
    fn open_with_device_id_returns_selected_system_audio_unavailable_for_unknown_id() {
        let device_id =
            AudioDeviceId::new("gijirec-nonexistent-output-id-xyz".to_string()).expect("valid id");

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
        let host = cpal::default_host();
        let default_device = match host.default_output_device() {
            Some(device) => device,
            None => return,
        };
        let name = match default_device.name() {
            Ok(name) => name,
            Err(_) => return,
        };
        let device_id = AudioDeviceId::new(name).expect("valid id");

        let found = find_output_device(&host, &device_id).expect("device found");
        assert_eq!(found.name().expect("name"), device_id.as_str());
    }

    // Integration Test 1 (WindowsLoopbackAdapter): default WASAPI loopback opens on hardware
    #[test]
    #[ignore = "CI: requires Windows default WASAPI loopback output device; run with --ignored on local hardware"]
    fn opens_default_loopback_on_hardware() {
        let (_adapter, mut consumer, sample_rate_hz) =
            WindowsLoopbackAdapter::open(DEFAULT_RING_CAPACITY).expect("default loopback");
        assert!(sample_rate_hz > 0);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(consumer.slots() > 0 || consumer.pop().is_some());
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

        let (_adapter, mut consumer, sample_rate_hz) =
            WindowsLoopbackAdapter::open_with_device_id(Some(&device_id), DEFAULT_RING_CAPACITY)
                .expect("non-default loopback");
        assert!(sample_rate_hz > 0);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(consumer.slots() > 0 || consumer.pop().is_some());
    }
}
