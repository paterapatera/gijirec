//! OS-specific audio adapters.

#[cfg(target_os = "windows")]
pub mod windows_loopback;

#[cfg(target_os = "macos")]
pub mod macos_sck_audio;

#[cfg(target_os = "windows")]
pub use windows_loopback::{LoopbackSampleConsumer, WindowsLoopbackAdapter};

#[cfg(target_os = "macos")]
pub use macos_sck_audio::{MacScreenCaptureKitAdapter, SckAudioSampleConsumer};
