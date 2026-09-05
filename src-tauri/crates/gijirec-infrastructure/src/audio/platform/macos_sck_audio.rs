//! macOS ScreenCaptureKit system audio adapter.

#[cfg(target_os = "macos")]
mod imp {
    use gijirec_domain::audio::CaptureError;
    use rtrb::RingBuffer;
    use screencapturekit::prelude::*;
    use screencapturekit::stream::output::SCStreamOutputTrait;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::audio::mic_capture::DEFAULT_RING_CAPACITY;

    /// SCK system audio adapter streaming f32 mono samples into an rtrb consumer.
    pub struct MacScreenCaptureKitAdapter {
        stream: SCStream,
        running: Arc<AtomicBool>,
    }

    /// Consumer side of the SCK audio ring buffer.
    pub struct SckAudioSampleConsumer {
        inner: rtrb::Consumer<f32>,
    }

    impl SckAudioSampleConsumer {
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

    struct AudioOutputHandler {
        producer: Mutex<rtrb::Producer<f32>>,
        permission_denied: Arc<AtomicBool>,
    }

    impl SCStreamOutputTrait for AudioOutputHandler {
        fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
            if of_type != SCStreamOutputType::Audio {
                return;
            }

            let Some(audio_buffer) = sample
                .audio_buffer_list()
                .and_then(|list| list.buffers().first().cloned())
            else {
                return;
            };

            let channels = audio_buffer.number_channels().max(1) as usize;
            let bytes = audio_buffer.data();
            if bytes.is_empty() {
                return;
            }

            let frame_count = bytes.len() / (channels * std::mem::size_of::<f32>());
            if frame_count == 0 {
                return;
            }

            let samples: &[f32] = unsafe {
                std::slice::from_raw_parts(bytes.as_ptr() as *const f32, frame_count * channels)
            };

            if let Ok(mut producer) = self.producer.lock() {
                push_mono_f32_rt(samples, channels, &mut producer);
            }
        }

        fn stream_did_stop_with_error(&self, error: Option<SCStreamError>) {
            if is_permission_denied_error(error.as_ref()) {
                self.permission_denied.store(true, Ordering::SeqCst);
            }
        }
    }

    impl MacScreenCaptureKitAdapter {
        pub fn open(
            ring_capacity: usize,
        ) -> Result<(Self, SckAudioSampleConsumer, u32), CaptureError> {
            let content = SCShareableContent::get().map_err(map_shareable_content_error)?;
            let display = content
                .displays()
                .first()
                .ok_or(CaptureError::SystemAudioUnavailable)?
                .clone();

            let filter = SCContentFilter::create()
                .with_display(&display)
                .with_excluding_windows(&[])
                .build();

            let config = SCStreamConfiguration::new()
                .with_width(2)
                .with_height(2)
                .with_minimum_frame_interval(std::time::Duration::from_secs(1))
                .with_captures_audio(true)
                .with_excludes_current_process_audio(true);

            let (producer, consumer) = RingBuffer::<f32>::new(ring_capacity);
            let permission_denied = Arc::new(AtomicBool::new(false));
            let handler = AudioOutputHandler {
                producer: Mutex::new(producer),
                permission_denied: Arc::clone(&permission_denied),
            };

            let mut stream = SCStream::new(&filter, &config);
            stream
                .add_output_handler(handler, SCStreamOutputType::Audio)
                .map_err(map_stream_error)?;

            stream.start_capture().map_err(|err| {
                if is_permission_denied_error(Some(&err)) {
                    CaptureError::SystemAudioPermissionDenied
                } else {
                    map_stream_start_error(err)
                }
            })?;

            if permission_denied.load(Ordering::SeqCst) {
                return Err(CaptureError::SystemAudioPermissionDenied);
            }

            Ok((
                Self {
                    stream,
                    running: Arc::new(AtomicBool::new(true)),
                },
                SckAudioSampleConsumer { inner: consumer },
                48_000,
            ))
        }
    }

    impl Drop for MacScreenCaptureKitAdapter {
        fn drop(&mut self) {
            self.running.store(false, Ordering::SeqCst);
            let _ = self.stream.stop_capture();
        }
    }

    fn push_mono_f32_rt(data: &[f32], channels: usize, producer: &mut rtrb::Producer<f32>) {
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

    fn is_permission_denied_error(error: Option<&SCStreamError>) -> bool {
        let Some(error) = error else {
            return false;
        };
        let message = error.to_string().to_ascii_lowercase();
        message.contains("permission")
            || message.contains("not authorized")
            || message.contains("denied")
            || message.contains("declined")
    }

    fn map_shareable_content_error(err: SCShareableContentError) -> CaptureError {
        let message = err.to_string().to_ascii_lowercase();
        if message.contains("permission") || message.contains("not authorized") {
            CaptureError::SystemAudioPermissionDenied
        } else {
            CaptureError::SystemAudioUnavailable
        }
    }

    fn map_stream_error(err: SCStreamError) -> CaptureError {
        if is_permission_denied_error(Some(&err)) {
            CaptureError::SystemAudioPermissionDenied
        } else {
            CaptureError::Internal {
                detail: err.to_string(),
            }
        }
    }

    fn map_stream_start_error(err: SCStreamError) -> CaptureError {
        if is_permission_denied_error(Some(&err)) {
            CaptureError::SystemAudioPermissionDenied
        } else {
            CaptureError::SystemAudioUnavailable
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::{MacScreenCaptureKitAdapter, SckAudioSampleConsumer};

#[cfg(not(target_os = "macos"))]
use gijirec_domain::audio::CaptureError;

#[cfg(not(target_os = "macos"))]
/// Non-macOS stub — compiled only for cross-target type checking.
pub struct MacScreenCaptureKitAdapter;

#[cfg(not(target_os = "macos"))]
/// Non-macOS stub consumer.
pub struct SckAudioSampleConsumer;

#[cfg(not(target_os = "macos"))]
impl MacScreenCaptureKitAdapter {
    pub fn open(_ring_capacity: usize) -> Result<(Self, SckAudioSampleConsumer), CaptureError> {
        Err(CaptureError::SystemAudioUnavailable)
    }
}

#[cfg(not(target_os = "macos"))]
impl SckAudioSampleConsumer {
    pub fn pop(&mut self) -> Option<f32> {
        None
    }

    pub fn drain_into(&mut self, _out: &mut [f32]) -> usize {
        0
    }

    pub fn slots(&self) -> usize {
        0
    }
}

#[cfg(target_os = "macos")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mic_capture::DEFAULT_RING_CAPACITY;

    /// Documented skip reason for macOS ScreenCaptureKit hardware tests (Integration Test 2).
    /// Must match `#[ignore = "..."]` on `opens_sck_audio_on_hardware` exactly.
    pub(crate) const MACOS_SCK_HARDWARE_SKIP: &str = "CI: requires macOS 13+ ScreenCaptureKit screen recording permission; run with --ignored on local hardware";

    #[test]
    fn documents_sck_hardware_ci_skip_reason() {
        let reason = MACOS_SCK_HARDWARE_SKIP;
        assert_eq!(
            reason,
            "CI: requires macOS 13+ ScreenCaptureKit screen recording permission; run with --ignored on local hardware"
        );
        assert!(reason.contains("ScreenCaptureKit"));
        assert!(reason.contains("permission"));
        assert!(reason.contains("CI"));
    }

    // Integration Test 2 (MacScreenCaptureKitAdapter): SCK system audio on hardware
    #[test]
    #[ignore = "CI: requires macOS 13+ ScreenCaptureKit screen recording permission; run with --ignored on local hardware"]
    fn opens_sck_audio_on_hardware() {
        let (_adapter, mut consumer, sample_rate_hz) =
            MacScreenCaptureKitAdapter::open(DEFAULT_RING_CAPACITY).expect("sck audio");
        assert_eq!(sample_rate_hz, 48_000);
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(consumer.slots() > 0 || consumer.pop().is_some());
    }
}
