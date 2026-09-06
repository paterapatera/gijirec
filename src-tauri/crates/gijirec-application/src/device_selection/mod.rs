//! Session-scoped audio device selection (application layer).

pub mod observability;
pub mod service;
pub mod store;

pub use observability::{
    DeviceSelectionObservability, NoopDeviceSelectionObservability,
    RecordingDeviceSelectionObservability,
};
pub use service::{
    CaptureSelectionPort, DefaultDeviceSelectionService, DeviceEnumeratorPort,
    DeviceSelectionClock, DeviceSelectionError, DeviceSelectionErrorCode, DeviceSelectionEvents,
    DeviceSelectionService, HOTPLUG_POLL_INTERVAL_MS, MacosSpeakerPreflight,
    NoopDeviceSelectionEvents, NoopSpeakerPreflight, SpeakerPreflightPort, SystemClock,
};
pub use store::DeviceSelectionStore;
