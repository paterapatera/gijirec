//! Microphone capture adapter using cpal with RT-safe rtrb output.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BuildStreamError, Device, SampleFormat, Stream, StreamConfig};
use gijirec_domain::audio::{AudioDeviceId, CaptureError};
use rtrb::RingBuffer;
use std::sync::Arc;

/// Notifies when a live input stream fails at runtime (req 4.3).
pub type StreamRuntimeErrorCallback = Arc<dyn Fn() + Send + Sync>;

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
        Self::open_with_device_id(None, ring_capacity)
    }

    /// Opens an input device by session id (`None` = OS default).
    pub fn open_with_device_id(
        device_id: Option<&AudioDeviceId>,
        ring_capacity: usize,
    ) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        Self::open_with_device_id_and_runtime_hook(device_id, ring_capacity, None)
    }

    pub fn open_with_device_id_and_runtime_hook(
        device_id: Option<&AudioDeviceId>,
        ring_capacity: usize,
        on_stream_error: Option<StreamRuntimeErrorCallback>,
    ) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        let host = cpal::default_host();
        Self::open_with_device_id_on_host(&host, device_id, ring_capacity, on_stream_error)
    }

    pub(crate) fn open_with_device_id_on_host<H: HostTrait<Device = Device>>(
        host: &H,
        device_id: Option<&AudioDeviceId>,
        ring_capacity: usize,
        on_stream_error: Option<StreamRuntimeErrorCallback>,
    ) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        let selected = device_id.is_some();
        let device = match device_id {
            None => host
                .default_input_device()
                .ok_or(CaptureError::MicUnavailable)?,
            Some(id) => find_input_device(host, id)?,
        };
        Self::open_device_inner(&device, selected, ring_capacity, on_stream_error)
    }

    /// Opens a specific cpal input device handle directly.
    ///
    /// For user-selected devices (session `AudioDeviceId`), use [`Self::open_with_device_id`]
    /// so missing IDs map to `CaptureError::SelectedMicUnavailable`.
    pub fn open_device(
        device: &Device,
        ring_capacity: usize,
    ) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        Self::open_device_inner(device, false, ring_capacity, None)
    }

    fn open_device_inner(
        device: &Device,
        selected: bool,
        ring_capacity: usize,
        on_stream_error: Option<StreamRuntimeErrorCallback>,
    ) -> Result<(Self, MicSampleConsumer, u32), CaptureError> {
        let supported = device
            .default_input_config()
            .map_err(|err| map_config_error(err, selected))?;
        let config: StreamConfig = supported.clone().into();
        let sample_rate_hz = config.sample_rate.0;
        let channels = supported.channels() as usize;
        let (producer, consumer) = RingBuffer::<f32>::new(ring_capacity);

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_f32_stream(
                device,
                &config,
                channels,
                producer,
                selected,
                on_stream_error,
            )?,
            SampleFormat::I16 => build_i16_stream(
                device,
                &config,
                channels,
                producer,
                selected,
                on_stream_error,
            )?,
            SampleFormat::U16 => build_u16_stream(
                device,
                &config,
                channels,
                producer,
                selected,
                on_stream_error,
            )?,
            other => {
                return Err(CaptureError::Internal {
                    detail: format!("unsupported input sample format: {other:?}"),
                });
            }
        };

        stream.play().map_err(|err| map_play_error(err, selected))?;

        Ok((
            Self { stream },
            MicSampleConsumer { inner: consumer },
            sample_rate_hz,
        ))
    }
}

/// Resolves a listed input device by cpal session id (`Device::name()`).
pub(crate) fn find_input_device<H: HostTrait<Device = Device>>(
    host: &H,
    device_id: &AudioDeviceId,
) -> Result<Device, CaptureError> {
    let target = device_id.as_str();
    let devices = host.input_devices().map_err(|err| CaptureError::Internal {
        detail: format!("failed to enumerate input devices: {err}"),
    })?;

    for device in devices {
        match device.name() {
            Ok(name) if name == target => return Ok(device),
            Ok(_) => continue,
            Err(err) => {
                return Err(CaptureError::Internal {
                    detail: format!("failed to read input device name: {err}"),
                });
            }
        }
    }

    Err(CaptureError::SelectedMicUnavailable)
}

#[allow(clippy::too_many_arguments)]
fn build_f32_stream(
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
            move |err| invoke_stream_runtime_error("mic", &err, on_stream_error.as_ref()),
            None,
        )
        .map_err(|err| map_build_error(err, selected))
}

#[allow(clippy::too_many_arguments)]
fn build_i16_stream(
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
            move |err| invoke_stream_runtime_error("mic", &err, on_stream_error.as_ref()),
            None,
        )
        .map_err(|err| map_build_error(err, selected))
}

#[allow(clippy::too_many_arguments)]
fn build_u16_stream(
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
            move |err| invoke_stream_runtime_error("mic", &err, on_stream_error.as_ref()),
            None,
        )
        .map_err(|err| map_build_error(err, selected))
}

fn invoke_stream_runtime_error(
    port: &str,
    err: &cpal::StreamError,
    on_stream_error: Option<&StreamRuntimeErrorCallback>,
) {
    let _ = (port, err);
    if let Some(callback) = on_stream_error {
        callback();
    }
}

pub(crate) fn invoke_loopback_stream_runtime_error(
    port: &str,
    err: &cpal::StreamError,
    on_stream_error: Option<&StreamRuntimeErrorCallback>,
) {
    invoke_stream_runtime_error(port, err, on_stream_error);
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

fn map_config_error(err: cpal::DefaultStreamConfigError, selected: bool) -> CaptureError {
    if selected {
        CaptureError::SelectedMicUnavailable
    } else {
        CaptureError::Internal {
            detail: format!("failed to query default input config: {err}"),
        }
    }
}

fn map_build_error(err: BuildStreamError, selected: bool) -> CaptureError {
    if selected {
        return match err {
            BuildStreamError::DeviceNotAvailable | BuildStreamError::StreamConfigNotSupported => {
                CaptureError::SelectedMicUnavailable
            }
            BuildStreamError::InvalidArgument => CaptureError::Internal {
                detail: err.to_string(),
            },
            BuildStreamError::BackendSpecific { err } => {
                if is_permission_denied(&err) {
                    CaptureError::MicPermissionDenied
                } else {
                    CaptureError::SelectedMicUnavailable
                }
            }
            other => CaptureError::Internal {
                detail: other.to_string(),
            },
        };
    }

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

fn map_play_error(err: cpal::PlayStreamError, selected: bool) -> CaptureError {
    if selected {
        return match err {
            cpal::PlayStreamError::DeviceNotAvailable => CaptureError::SelectedMicUnavailable,
            cpal::PlayStreamError::BackendSpecific { err } => {
                if is_permission_denied(&err) {
                    CaptureError::MicPermissionDenied
                } else {
                    CaptureError::SelectedMicUnavailable
                }
            }
        };
    }

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

// cpal 0.16 の CoreAudio `Stream` は property listener 用 `Box<dyn FnMut()>` を
// 保持しており Rust 上 `Send` にならない。オーケストレータ Mutex 配下でのみ
// open/close し、サンプル配信は lock-free rtrb と `Send + Sync` な runtime hook のみ。
#[cfg(target_os = "macos")]
unsafe impl Send for MicCaptureAdapter {}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::AudioDeviceId;

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
    fn find_input_device_returns_selected_mic_unavailable_for_unknown_id() {
        let host = cpal::default_host();
        let device_id =
            AudioDeviceId::new("gijirec-nonexistent-mic-id-xyz".to_string()).expect("valid id");

        assert!(matches!(
            find_input_device(&host, &device_id),
            Err(CaptureError::SelectedMicUnavailable)
        ));
    }

    #[test]
    fn open_with_device_id_returns_selected_mic_unavailable_for_unknown_id() {
        let device_id =
            AudioDeviceId::new("gijirec-nonexistent-mic-id-xyz".to_string()).expect("valid id");

        let err = MicCaptureAdapter::open_with_device_id(Some(&device_id), DEFAULT_RING_CAPACITY)
            .err()
            .expect("error");
        assert_eq!(err, CaptureError::SelectedMicUnavailable);
        assert_eq!(
            err.to_user_facing().code.as_str(),
            "SELECTED_MIC_UNAVAILABLE"
        );
    }

    #[test]
    fn find_input_device_resolves_device_when_name_matches() {
        let host = cpal::default_host();
        let default_device = match host.default_input_device() {
            Some(device) => device,
            None => return,
        };
        let name = match default_device.name() {
            Ok(name) => name,
            Err(_) => return,
        };
        let device_id = AudioDeviceId::new(name).expect("valid id");

        let found = find_input_device(&host, &device_id).expect("device found");
        assert_eq!(found.name().expect("name"), device_id.as_str());
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

    #[test]
    #[ignore = "requires default input device and OS mic permission"]
    fn opens_selected_input_device_on_hardware() {
        let host = cpal::default_host();
        let default_device = host.default_input_device().expect("default input device");
        let name = default_device.name().expect("device name");
        let device_id = AudioDeviceId::new(name).expect("valid device id");

        let (_adapter, mut consumer, sample_rate_hz) =
            MicCaptureAdapter::open_with_device_id(Some(&device_id), DEFAULT_RING_CAPACITY)
                .expect("selected default mic by name");
        assert!(sample_rate_hz > 0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(consumer.slots() > 0 || consumer.pop().is_some());
    }
}
