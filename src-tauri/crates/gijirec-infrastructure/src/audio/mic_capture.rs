//! Microphone capture adapter using cpal with RT-safe rtrb output.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BuildStreamError, Device, SampleFormat, Stream, StreamConfig};
use gijirec_domain::audio::CaptureError;
use rtrb::RingBuffer;

/// Default ring buffer capacity for mic samples (f32 mono).
pub const DEFAULT_RING_CAPACITY: usize = 8_192;

/// Mic capture adapter streaming f32 mono samples into an rtrb consumer.
pub struct MicCaptureAdapter {
    /// Keeps the cpal input stream alive for the lifetime of the adapter.
    #[expect(dead_code)]
    stream: Stream,
}

/// Consumer side of the mic capture ring buffer.
pub struct MicSampleConsumer {
    inner: rtrb::Consumer<f32>,
}

impl MicSampleConsumer {
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

impl MicCaptureAdapter {
    /// Opens the default input device and starts streaming mono f32 samples.
    pub fn open(ring_capacity: usize) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(CaptureError::MicUnavailable)?;
        Self::open_device(&device, ring_capacity)
    }

    /// Opens a specific input device (used in tests and composition root).
    pub fn open_device(
        device: &Device,
        ring_capacity: usize,
    ) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        let supported = device.default_input_config().map_err(map_config_error)?;
        let config: StreamConfig = supported.clone().into();
        let sample_rate_hz = config.sample_rate.0;
        let channels = supported.channels() as usize;
        let (producer, consumer) = RingBuffer::<f32>::new(ring_capacity);

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_f32_stream(device, &config, channels, producer)?,
            SampleFormat::I16 => build_i16_stream(device, &config, channels, producer)?,
            SampleFormat::U16 => build_u16_stream(device, &config, channels, producer)?,
            other => {
                return Err(CaptureError::Internal {
                    detail: format!("unsupported input sample format: {other:?}"),
                });
            }
        };

        stream.play().map_err(map_play_error)?;

        Ok((
            Self { stream },
            MicSampleConsumer { inner: consumer },
            sample_rate_hz,
        ))
    }
}

fn build_f32_stream(
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

fn build_i16_stream(
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

fn build_u16_stream(
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

pub(crate) fn push_mono_f32(data: &[f32], channels: usize, producer: &mut rtrb::Producer<f32>) {
    if channels <= 1 {
        for sample in data {
            let _ = producer.push(*sample);
        }
        return;
    }

    let frames = data.len() / channels;
    for frame in 0..frames {
        let base = frame * channels;
        let mut sum = 0.0_f32;
        for ch in 0..channels {
            sum += data[base + ch];
        }
        let _ = producer.push(sum / channels as f32);
    }
}

pub(crate) fn push_mono_i16(data: &[i16], channels: usize, producer: &mut rtrb::Producer<f32>) {
    if channels <= 1 {
        for sample in data {
            let _ = producer.push(i16_to_f32(*sample));
        }
        return;
    }

    let frames = data.len() / channels;
    for frame in 0..frames {
        let base = frame * channels;
        let mut sum = 0.0_f32;
        for ch in 0..channels {
            sum += i16_to_f32(data[base + ch]);
        }
        let _ = producer.push(sum / channels as f32);
    }
}

pub(crate) fn push_mono_u16(data: &[u16], channels: usize, producer: &mut rtrb::Producer<f32>) {
    if channels <= 1 {
        for sample in data {
            let _ = producer.push(u16_to_f32(*sample));
        }
        return;
    }

    let frames = data.len() / channels;
    for frame in 0..frames {
        let base = frame * channels;
        let mut sum = 0.0_f32;
        for ch in 0..channels {
            sum += u16_to_f32(data[base + ch]);
        }
        let _ = producer.push(sum / channels as f32);
    }
}

fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / i16::MAX as f32
}

fn u16_to_f32(sample: u16) -> f32 {
    (sample as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)
}

fn map_config_error(err: cpal::DefaultStreamConfigError) -> CaptureError {
    CaptureError::Internal {
        detail: format!("failed to query default input config: {err}"),
    }
}

fn map_build_error(err: BuildStreamError) -> CaptureError {
    match err {
        BuildStreamError::DeviceNotAvailable => CaptureError::MicUnavailable,
        BuildStreamError::StreamConfigNotSupported => CaptureError::MicUnavailable,
        BuildStreamError::InvalidArgument => CaptureError::Internal {
            detail: err.to_string(),
        },
        BuildStreamError::BackendSpecific { err } => {
            if is_permission_denied(&err) {
                CaptureError::MicPermissionDenied
            } else {
                CaptureError::Internal {
                    detail: err.to_string(),
                }
            }
        }
        other => CaptureError::Internal {
            detail: other.to_string(),
        },
    }
}

fn map_play_error(err: cpal::PlayStreamError) -> CaptureError {
    match err {
        cpal::PlayStreamError::DeviceNotAvailable => CaptureError::MicUnavailable,
        cpal::PlayStreamError::BackendSpecific { err } => {
            if is_permission_denied(&err) {
                CaptureError::MicPermissionDenied
            } else {
                CaptureError::Internal {
                    detail: err.to_string(),
                }
            }
        }
    }
}

fn is_permission_denied(err: &cpal::BackendSpecificError) -> bool {
    let message = err.to_string().to_ascii_lowercase();
    message.contains("permission")
        || message.contains("access denied")
        || message.contains("not authorized")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_mono_downmixes_stereo_frames() {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
        let samples = [0.5_f32, -0.5_f32, 1.0_f32, 1.0_f32];
        push_mono_f32(&samples, 2, &mut producer);
        let mut out = [0.0_f32; 2];
        let mut count = 0;
        for slot in out.iter_mut() {
            if let Ok(sample) = consumer.pop() {
                *slot = sample;
                count += 1;
            }
        }
        assert_eq!(count, 2);
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 1.0);
    }

    #[test]
    #[ignore = "requires default input device and OS mic permission"]
    fn opens_default_input_device_on_hardware() {
        let (_adapter, mut consumer, sample_rate_hz) =
            MicCaptureAdapter::open(DEFAULT_RING_CAPACITY).expect("default mic");
        assert!(sample_rate_hz > 0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(consumer.slots() > 0 || consumer.pop().is_some());
    }
}
