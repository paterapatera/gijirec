//! Shared cpal mono-downmix input stream builders and PCM push helpers.

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{
    BuildStreamError, Device, PlayStreamError, SampleFormat, Stream, StreamConfig,
    SupportedStreamConfig,
};
use gijirec_domain::audio::{AudioDeviceId, CaptureError};
use std::sync::Arc;

/// Notifies when a live input stream fails at runtime (req 4.3).
pub type StreamRuntimeErrorCallback = Arc<dyn Fn() + Send + Sync>;

type MapBuildError = fn(BuildStreamError, bool) -> CaptureError;
pub(crate) type MapPlayError = fn(cpal::PlayStreamError, bool) -> CaptureError;

pub(crate) fn invoke_stream_runtime_error(
    port: &str,
    err: &cpal::StreamError,
    on_stream_error: Option<&StreamRuntimeErrorCallback>,
) {
    let _ = (port, err);
    if let Some(callback) = on_stream_error {
        callback();
    }
}

fn push_mono_samples<S, F>(
    data: &[S],
    channels: usize,
    producer: &mut rtrb::Producer<f32>,
    to_f32: F,
) where
    S: Copy,
    F: Fn(S) -> f32,
{
    if channels <= 1 {
        for sample in data {
            let _ = producer.push(to_f32(*sample));
        }
        return;
    }

    let frames = data.len() / channels;
    for frame in 0..frames {
        let base = frame * channels;
        let mut sum = 0.0_f32;
        for ch in 0..channels {
            sum += to_f32(data[base + ch]);
        }
        let _ = producer.push(sum / channels as f32);
    }
}

pub(crate) fn push_mono_f32(data: &[f32], channels: usize, producer: &mut rtrb::Producer<f32>) {
    push_mono_samples(data, channels, producer, |sample| sample);
}

pub(crate) fn push_mono_i16(data: &[i16], channels: usize, producer: &mut rtrb::Producer<f32>) {
    push_mono_samples(data, channels, producer, i16_to_f32);
}

pub(crate) fn push_mono_u16(data: &[u16], channels: usize, producer: &mut rtrb::Producer<f32>) {
    push_mono_samples(data, channels, producer, u16_to_f32);
}

fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / i16::MAX as f32
}

fn u16_to_f32(sample: u16) -> f32 {
    (sample as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)
}

macro_rules! impl_build_mono_input_stream {
    ($fn_name:ident, $sample_ty:ty, $push_fn:ident) => {
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn $fn_name(
            device: &Device,
            config: &StreamConfig,
            channels: usize,
            mut producer: rtrb::Producer<f32>,
            port: &'static str,
            selected: bool,
            on_stream_error: Option<StreamRuntimeErrorCallback>,
            map_build_error: MapBuildError,
        ) -> Result<Stream, CaptureError> {
            device
                .build_input_stream(
                    config,
                    move |data: &[$sample_ty], _| $push_fn(data, channels, &mut producer),
                    move |err| invoke_stream_runtime_error(port, &err, on_stream_error.as_ref()),
                    None,
                )
                .map_err(|err| map_build_error(err, selected))
        }
    };
}

impl_build_mono_input_stream!(build_f32_input_stream, f32, push_mono_f32);
impl_build_mono_input_stream!(build_i16_input_stream, i16, push_mono_i16);
impl_build_mono_input_stream!(build_u16_input_stream, u16, push_mono_u16);

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_and_play_mono_input_stream(
    device: &Device,
    supported: &SupportedStreamConfig,
    ring_capacity: usize,
    port: &'static str,
    selected: bool,
    on_stream_error: Option<StreamRuntimeErrorCallback>,
    map_build_error: MapBuildError,
    map_play_error: MapPlayError,
    unsupported_format_detail: impl FnOnce(SampleFormat) -> String,
) -> Result<(Stream, rtrb::Consumer<f32>, u32), CaptureError> {
    let config: StreamConfig = supported.clone().into();
    let sample_rate_hz = config.sample_rate.0;
    let channels = supported.channels() as usize;
    let (producer, consumer) = rtrb::RingBuffer::<f32>::new(ring_capacity);

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_f32_input_stream(
            device,
            &config,
            channels,
            producer,
            port,
            selected,
            on_stream_error.clone(),
            map_build_error,
        )?,
        SampleFormat::I16 => build_i16_input_stream(
            device,
            &config,
            channels,
            producer,
            port,
            selected,
            on_stream_error.clone(),
            map_build_error,
        )?,
        SampleFormat::U16 => build_u16_input_stream(
            device,
            &config,
            channels,
            producer,
            port,
            selected,
            on_stream_error,
            map_build_error,
        )?,
        other => {
            return Err(CaptureError::Internal {
                detail: unsupported_format_detail(other),
            });
        }
    };

    stream.play().map_err(|err| map_play_error(err, selected))?;
    Ok((stream, consumer, sample_rate_hz))
}

pub(crate) fn is_permission_denied(err: &cpal::BackendSpecificError) -> bool {
    let message = err.to_string().to_ascii_lowercase();
    message.contains("permission")
        || message.contains("access denied")
        || message.contains("not authorized")
}

#[allow(clippy::too_many_arguments)]
fn unavailable_for_selected<T>(selected: bool, selected_unavailable: T, unavailable: T) -> T {
    if selected {
        selected_unavailable
    } else {
        unavailable
    }
}

fn map_device_unavailable(
    selected: bool,
    selected_unavailable: CaptureError,
    unavailable: CaptureError,
) -> CaptureError {
    unavailable_for_selected(selected, selected_unavailable, unavailable)
}

fn map_backend_specific(
    err: cpal::BackendSpecificError,
    map_backend: impl FnOnce(cpal::BackendSpecificError) -> CaptureError,
) -> CaptureError {
    map_backend(err)
}

pub(crate) enum CpalStreamError {
    Build(BuildStreamError),
    Play(PlayStreamError),
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn map_stream_error(
    err: CpalStreamError,
    selected: bool,
    selected_unavailable: CaptureError,
    unavailable: CaptureError,
    map_backend: impl FnOnce(cpal::BackendSpecificError) -> CaptureError,
) -> CaptureError {
    match err {
        CpalStreamError::Build(err) => match err {
            BuildStreamError::DeviceNotAvailable | BuildStreamError::StreamConfigNotSupported => {
                map_device_unavailable(selected, selected_unavailable, unavailable)
            }
            BuildStreamError::InvalidArgument => CaptureError::Internal {
                detail: err.to_string(),
            },
            BuildStreamError::BackendSpecific { err } => map_backend_specific(err, map_backend),
            other => CaptureError::Internal {
                detail: other.to_string(),
            },
        },
        CpalStreamError::Play(err) => match err {
            PlayStreamError::DeviceNotAvailable => {
                map_device_unavailable(selected, selected_unavailable, unavailable)
            }
            PlayStreamError::BackendSpecific { err } => map_backend_specific(err, map_backend),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_device_matching_id<D, E>(
    devices: impl IntoIterator<Item = D>,
    device_id: &AudioDeviceId,
    name: impl Fn(&D) -> Result<String, E>,
    map_name_error: impl Fn(E) -> CaptureError,
    not_found: CaptureError,
) -> Result<D, CaptureError> {
    let target = device_id.as_str();
    for device in devices {
        match name(&device) {
            Ok(name) if name == target => return Ok(device),
            Ok(_) => continue,
            Err(err) => return Err(map_name_error(err)),
        }
    }
    Err(not_found)
}
