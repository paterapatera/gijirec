use std::sync::{Arc, Mutex};

use gijirec_presentation::application::capture_audio_controls::CaptureAudioControlsEvents;
use gijirec_presentation::application::device_selection::DeviceSelectionEvents;
use gijirec_presentation::domain::audio::{AudioDeviceList, CaptureError, DeviceSelection};

struct LateBoundEmitter<T: ?Sized> {
    slot: Mutex<Option<Arc<T>>>,
}

impl<T: ?Sized> LateBoundEmitter<T> {
    fn new() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }

    fn set(&self, emitter: Arc<T>) {
        *self.slot.lock().expect("lock") = Some(emitter);
    }

    fn get(&self) -> Option<Arc<T>> {
        self.slot.lock().expect("lock").clone()
    }
}

macro_rules! late_bound_events_shell {
    ($name:ident, $trait:path) => {
        pub(crate) struct $name {
            inner: LateBoundEmitter<dyn $trait>,
        }

        impl $name {
            pub(crate) fn new() -> Self {
                Self {
                    inner: LateBoundEmitter::new(),
                }
            }

            pub(crate) fn set_emitter(&self, emitter: Arc<dyn $trait>) {
                self.inner.set(emitter);
            }
        }
    };
}

late_bound_events_shell!(LateBoundDeviceSelectionEvents, DeviceSelectionEvents);

impl DeviceSelectionEvents for LateBoundDeviceSelectionEvents {
    fn emit_selection_changed(&self, selection: &DeviceSelection) {
        if let Some(emitter) = self.inner.get() {
            emitter.emit_selection_changed(selection);
        }
    }

    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64) {
        if let Some(emitter) = self.inner.get() {
            emitter.emit_devices_changed(devices, timestamp_ms);
        }
    }
}

/// Forwards [`DeviceSelectionEvents`] to a shared [`LateBoundDeviceSelectionEvents`].
pub(crate) struct DeviceSelectionEventsProxy(pub(crate) Arc<LateBoundDeviceSelectionEvents>);

impl DeviceSelectionEvents for DeviceSelectionEventsProxy {
    fn emit_selection_changed(&self, selection: &DeviceSelection) {
        self.0.emit_selection_changed(selection);
    }

    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64) {
        self.0.emit_devices_changed(devices, timestamp_ms);
    }
}

late_bound_events_shell!(
    LateBoundCaptureAudioControlsEvents,
    CaptureAudioControlsEvents
);

impl CaptureAudioControlsEvents for LateBoundCaptureAudioControlsEvents {
    fn emit_controls_changed(
        &self,
        controls: &gijirec_presentation::domain::audio::CaptureAudioControls,
    ) {
        if let Some(emitter) = self.inner.get() {
            emitter.emit_controls_changed(controls);
        }
    }

    fn emit_capture_error(&self, error: CaptureError) {
        if let Some(emitter) = self.inner.get() {
            emitter.emit_capture_error(error);
        }
    }
}

pub(crate) struct CaptureAudioControlsEventsProxy(
    pub(crate) Arc<LateBoundCaptureAudioControlsEvents>,
);

impl CaptureAudioControlsEvents for CaptureAudioControlsEventsProxy {
    fn emit_controls_changed(
        &self,
        controls: &gijirec_presentation::domain::audio::CaptureAudioControls,
    ) {
        self.0.emit_controls_changed(controls);
    }

    fn emit_capture_error(&self, error: CaptureError) {
        self.0.emit_capture_error(error);
    }
}
