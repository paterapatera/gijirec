//! Windows WASAPI loopback adapter using cpal on the default output device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BuildStreamError, Device, SampleFormat, Stream, StreamConfig};
use gijirec_domain::audio::CaptureError;
use rtrb::RingBuffer;

use crate::audio::mic_capture::{push_mono_f32, push_mono_i16, push_mono_u16};

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
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(CaptureError::SystemAudioUnavailable)?;
        Self::open_device(&device, ring_capacity)
    }

    /// Opens a specific output device (used in tests and composition root).
    pub fn open_device(
        device: &Device,
        ring_capacity: usize,
    ) -> Result<(Self, LoopbackSampleConsumer, u32), CaptureError> {
        let supported = device
            .default_output_config()
            .map_err(map_output_config_error)?;
        let config: StreamConfig = supported.clone().into();
        let sample_rate_hz = config.sample_rate.0;
        let channels = supported.channels() as usize;
        let (producer, consumer) = RingBuffer::<f32>::new(ring_capacity);

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_f32_loopback(device, &config, channels, producer)?,
            SampleFormat::I16 => build_i16_loopback(device, &config, channels, producer)?,
            SampleFormat::U16 => build_u16_loopback(device, &config, channels, producer)?,
            other => {
                return Err(CaptureError::Internal {
                    detail: format!("unsupported loopback sample format: {other:?}"),
                });
            }
        };

        stream.play().map_err(map_play_error)?;

        Ok((
            Self { stream },
            LoopbackSampleConsumer { inner: consumer },
            sample_rate_hz,
        ))
    }
}

fn build_f32_loopback(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
) -> Result<Stream, CaptureError> {
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| push_mono_f32(data, channels, &mut producer),
            |_| {},
            None,
        )
        .map_err(map_build_error)
}

fn build_i16_loopback(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
) -> Result<Stream, CaptureError> {
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| push_mono_i16(data, channels, &mut producer),
            |_| {},
            None,
        )
        .map_err(map_build_error)
}

fn build_u16_loopback(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
) -> Result<Stream, CaptureError> {
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| push_mono_u16(data, channels, &mut producer),
            |_| {},
            None,
        )
        .map_err(map_build_error)
}

fn map_output_config_error(err: cpal::DefaultStreamConfigError) -> CaptureError {
    CaptureError::Internal {
        detail: format!("failed to query default output config: {err}"),
    }
}

fn map_build_error(err: BuildStreamError) -> CaptureError {
    match err {
        BuildStreamError::DeviceNotAvailable => CaptureError::SystemAudioUnavailable,
        BuildStreamError::StreamConfigNotSupported => CaptureError::SystemAudioUnavailable,
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

fn map_play_error(err: cpal::PlayStreamError) -> CaptureError {
    match err {
        cpal::PlayStreamError::DeviceNotAvailable => CaptureError::SystemAudioUnavailable,
        cpal::PlayStreamError::BackendSpecific { err } => CaptureError::Internal {
            detail: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mic_capture::DEFAULT_RING_CAPACITY;

    /// Documented skip reason for Windows WASAPI loopback hardware tests (Integration Test 1).
    /// Must match `#[ignore = "..."]` on `opens_default_loopback_on_hardware` exactly.
    pub(crate) const WINDOWS_LOOPBACK_HARDWARE_SKIP: &str = "CI: requires Windows default WASAPI loopback output device; run with --ignored on local hardware";

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
}
