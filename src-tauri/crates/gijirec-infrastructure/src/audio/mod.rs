//! Infrastructure audio adapters.
pub mod mic_capture;
pub mod platform;
pub mod resampler;

pub use mic_capture::{DEFAULT_RING_CAPACITY, MicCaptureAdapter, MicSampleConsumer};
pub use resampler::{
    MonoResampler, MonoResamplerPipeline, ResampledSampleConsumer, ResamplerInput,
    TARGET_SAMPLE_RATE_HZ,
};

#[cfg(target_os = "windows")]
pub use platform::{LoopbackSampleConsumer, WindowsLoopbackAdapter};

#[cfg(target_os = "macos")]
pub use platform::{MacScreenCaptureKitAdapter, SckAudioSampleConsumer};
