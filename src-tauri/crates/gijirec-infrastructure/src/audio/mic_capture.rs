//! Microphone capture adapter using cpal with RT-safe rtrb output.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BuildStreamError, Device, Stream};
use gijirec_domain::audio::{AudioDeviceId, CaptureError};

pub use super::cpal_mono_input::StreamRuntimeErrorCallback;

/// Default ring buffer capacity for mic samples (f32 mono).
pub const DEFAULT_RING_CAPACITY: usize = 8_192;

/// Mic capture adapter streaming f32 mono samples into an rtrb consumer.
pub struct MicCaptureAdapter {
    /// Keeps the cpal input stream alive for the lifetime of the adapter.
    #[expect(dead_code)]
    stream: Stream,
}

/// Consumer side of the mic capture ring buffer.
pub type MicSampleConsumer = super::f32_ring_consumer::F32RingConsumer;

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
        let (stream, consumer, sample_rate_hz) =
            super::cpal_mono_input::build_and_play_mono_input_stream(
                device,
                &supported,
                ring_capacity,
                "mic",
                selected,
                on_stream_error,
                map_build_error,
                map_play_error,
                |other| format!("unsupported input sample format: {other:?}"),
            )?;

        Ok((
            Self { stream },
            MicSampleConsumer::from_ring_consumer(consumer),
            sample_rate_hz,
        ))
    }
}

/// Resolves a listed input device by cpal session id (`Device::name()`).
pub(crate) fn find_input_device<H: HostTrait<Device = Device>>(
    host: &H,
    device_id: &AudioDeviceId,
) -> Result<Device, CaptureError> {
    super::cpal_mono_input::find_device_matching_id(
        host.input_devices().map_err(|err| CaptureError::Internal {
            detail: format!("failed to enumerate input devices: {err}"),
        })?,
        device_id,
        |device| device.name(),
        |err| CaptureError::Internal {
            detail: format!("failed to read input device name: {err}"),
        },
        CaptureError::SelectedMicUnavailable,
    )
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

fn map_mic_backend_error(err: cpal::BackendSpecificError, selected: bool) -> CaptureError {
    if super::cpal_mono_input::is_permission_denied(&err) {
        CaptureError::MicPermissionDenied
    } else if selected {
        CaptureError::SelectedMicUnavailable
    } else {
        CaptureError::Internal {
            detail: err.to_string(),
        }
    }
}

fn map_build_error(err: BuildStreamError, selected: bool) -> CaptureError {
    super::cpal_mono_input::map_stream_error(
        super::cpal_mono_input::CpalStreamError::Build(err),
        selected,
        CaptureError::SelectedMicUnavailable,
        CaptureError::MicUnavailable,
        |err| map_mic_backend_error(err, selected),
    )
}

fn map_play_error(err: cpal::PlayStreamError, selected: bool) -> CaptureError {
    super::cpal_mono_input::map_stream_error(
        super::cpal_mono_input::CpalStreamError::Play(err),
        selected,
        CaptureError::SelectedMicUnavailable,
        CaptureError::MicUnavailable,
        |err| map_mic_backend_error(err, selected),
    )
}

// cpal 0.16 の CoreAudio `Stream` は property listener 用 `Box<dyn FnMut()>` を
// 保持しており Rust 上 `Send` にならない。オーケストレータ Mutex 配下でのみ
// open/close し、サンプル配信は lock-free rtrb と `Send + Sync` な runtime hook のみ。
#[cfg(target_os = "macos")]
unsafe impl Send for MicCaptureAdapter {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::cpal_mono_input::push_mono_f32;
    use gijirec_domain::audio::AudioDeviceId;
    use rtrb::RingBuffer;

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
        crate::audio::cpal_device_test_support::assert_find_device_returns_error_for_unknown_id(
            find_input_device,
            CaptureError::SelectedMicUnavailable,
        );
    }

    #[test]
    fn open_with_device_id_returns_selected_mic_unavailable_for_unknown_id() {
        let device_id = crate::audio::cpal_device_test_support::unknown_device_id();

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
        use cpal::traits::HostTrait;

        crate::audio::cpal_device_test_support::assert_resolves_device_when_default_name_matches(
            |host| host.default_input_device(),
            find_input_device,
        );
    }

    #[test]
    #[ignore = "requires default input device and OS mic permission"]
    fn opens_default_input_device_on_hardware() {
        let (_adapter, consumer, sample_rate_hz) =
            MicCaptureAdapter::open(DEFAULT_RING_CAPACITY).expect("default mic");
        crate::audio::cpal_device_test_support::assert_cpal_consumer_receives_samples(
            sample_rate_hz,
            consumer,
            100,
        );
    }

    #[test]
    #[ignore = "requires default input device and OS mic permission"]
    fn opens_selected_input_device_on_hardware() {
        use cpal::traits::HostTrait;

        let host = cpal::default_host();
        let default_device = host.default_input_device().expect("default input device");
        let name = default_device.name().expect("device name");
        let device_id = AudioDeviceId::new(name).expect("valid device id");

        let (_adapter, consumer, sample_rate_hz) =
            MicCaptureAdapter::open_with_device_id(Some(&device_id), DEFAULT_RING_CAPACITY)
                .expect("selected default mic by name");
        crate::audio::cpal_device_test_support::assert_cpal_consumer_receives_samples(
            sample_rate_hz,
            consumer,
            100,
        );
    }
}
