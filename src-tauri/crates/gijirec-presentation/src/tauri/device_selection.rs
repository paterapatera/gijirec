//! Testable device selection command logic and Tauri event emission.
//!
//! Thin `#[tauri::command]` wrappers live in the host `commands` module (task 7.1).

use crate::application::capture::orchestrator::CaptureOrchestrator;
use crate::application::device_selection::{
    DeviceSelectionError, DeviceSelectionEvents, DeviceSelectionService,
};
use crate::tauri::events::CaptureEventEmitter;
use gijirec_domain::audio::{AudioDeviceList, CapturePhase, DeviceSelection};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};

/// Tauri event name for device list changes.
pub const DEVICES_CHANGED_EVENT: &str = "audio-device-selection://devices-changed";

/// Tauri event name for selection changes.
pub const SELECTION_CHANGED_EVENT: &str = "audio-device-selection://selection-changed";

/// Invoke error payload per `docs/contracts/audio-device-selection.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSelectionInvokeError {
    pub code: String,
    pub message_ja: String,
    pub action_ja: String,
}

impl DeviceSelectionInvokeError {
    pub fn from_service_error(err: DeviceSelectionError) -> Self {
        Self {
            code: err.code.as_str().to_string(),
            message_ja: err.message_ja,
            action_ja: err.action_ja,
        }
    }
}

/// Payload per `docs/contracts/audio-device-selection.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevicesChangedPayload {
    pub devices: AudioDeviceList,
    pub timestamp_ms: u64,
}

/// Payload per `docs/contracts/audio-device-selection.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSelectionChangedPayload {
    pub selection: DeviceSelection,
    pub timestamp_ms: u64,
}

pub fn build_devices_changed_payload(
    devices: &AudioDeviceList,
    timestamp_ms: u64,
) -> AudioDevicesChangedPayload {
    AudioDevicesChangedPayload {
        devices: devices.clone(),
        timestamp_ms,
    }
}

pub fn build_selection_changed_payload(
    selection: &DeviceSelection,
    timestamp_ms: u64,
) -> DeviceSelectionChangedPayload {
    DeviceSelectionChangedPayload {
        selection: selection.clone(),
        timestamp_ms,
    }
}

/// Production emitter backed by [`AppHandle`], implementing [`DeviceSelectionEvents`].
pub struct TauriDeviceSelectionEventEmitter<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriDeviceSelectionEventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> DeviceSelectionEvents for TauriDeviceSelectionEventEmitter<R> {
    fn emit_selection_changed(&self, selection: &DeviceSelection) {
        let payload = build_selection_changed_payload(
            selection,
            crate::tauri::invoke_contract::current_timestamp_ms(),
        );
        let _ = self.app.emit(SELECTION_CHANGED_EVENT, payload);
    }

    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64) {
        let payload = build_devices_changed_payload(devices, timestamp_ms);
        let _ = self.app.emit(DEVICES_CHANGED_EVENT, payload);
    }
}

/// In-memory recorder for unit tests.
#[derive(Debug, Default, Clone)]
pub struct RecordingDeviceSelectionEventEmitter {
    selections: Arc<Mutex<Vec<DeviceSelectionChangedPayload>>>,
    devices: Arc<Mutex<Vec<AudioDevicesChangedPayload>>>,
}

impl RecordingDeviceSelectionEventEmitter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selections(&self) -> Vec<DeviceSelectionChangedPayload> {
        self.selections.lock().expect("lock").clone()
    }

    pub fn devices(&self) -> Vec<AudioDevicesChangedPayload> {
        self.devices.lock().expect("lock").clone()
    }
}

impl DeviceSelectionEvents for RecordingDeviceSelectionEventEmitter {
    fn emit_selection_changed(&self, selection: &DeviceSelection) {
        self.selections
            .lock()
            .expect("lock")
            .push(build_selection_changed_payload(
                selection,
                crate::tauri::invoke_contract::current_timestamp_ms(),
            ));
    }

    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64) {
        self.devices
            .lock()
            .expect("lock")
            .push(build_devices_changed_payload(devices, timestamp_ms));
    }
}

/// Lists available audio devices.
pub fn list_audio_devices_impl(
    service: &dyn DeviceSelectionService,
) -> Result<AudioDeviceList, DeviceSelectionInvokeError> {
    service
        .list_devices()
        .map_err(DeviceSelectionInvokeError::from_service_error)
}

/// Returns the current session device selection.
pub fn get_device_selection_impl(service: &dyn DeviceSelectionService) -> DeviceSelection {
    service.get_selection()
}

/// Applies a device selection update.
pub fn set_device_selection_impl(
    service: &dyn DeviceSelectionService,
    selection: DeviceSelection,
) -> Result<DeviceSelection, DeviceSelectionInvokeError> {
    service
        .set_selection(selection)
        .map_err(DeviceSelectionInvokeError::from_service_error)
}

/// Notifies the service that the device selection UI visibility changed.
pub fn set_audio_device_ui_visible_impl(service: &dyn DeviceSelectionService, visible: bool) {
    service.set_ui_visible(visible);
}

/// Applies selection and emits capture phase/error events from orchestrator outcome.
///
/// Used by integration tests and the command path when restart surfaces a [`CaptureError`].
#[allow(clippy::too_many_arguments)]
pub fn set_device_selection_with_capture_feedback(
    service: &dyn DeviceSelectionService,
    orchestrator: &Arc<Mutex<dyn CaptureOrchestrator>>,
    emitter: &dyn CaptureEventEmitter,
    selection: DeviceSelection,
    restart_capture_error: impl FnOnce() -> Option<gijirec_domain::audio::CaptureError>,
) -> Result<DeviceSelection, DeviceSelectionInvokeError> {
    let result = set_device_selection_impl(service, selection);
    let phase = orchestrator.lock().expect("lock").phase();
    let restart_capture_error = restart_capture_error();
    match result {
        Ok(applied) => {
            if phase == CapturePhase::Capturing {
                let _ = emitter.emit_phase_changed(phase);
            }
            Ok(applied)
        }
        Err(err) => {
            if phase == CapturePhase::Error {
                if let Some(capture_err) = restart_capture_error {
                    let _ = emitter.emit_error(capture_err);
                }
                let _ = emitter.emit_phase_changed(CapturePhase::Error);
            }
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::AudioDeviceId;
    use gijirec_domain::audio::fixtures::sample_device_list;
    use gijirec_domain::user_facing_contract_tests::assert_invoke_error_serializes_contract_shape;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn sample_list() -> AudioDeviceList {
        sample_device_list()
    }

    struct MockDeviceSelectionService {
        list: AudioDeviceList,
        selection: DeviceSelection,
        list_error: Option<DeviceSelectionError>,
        set_error: Option<DeviceSelectionError>,
        ui_visible: AtomicBool,
    }

    impl MockDeviceSelectionService {
        fn new(list: AudioDeviceList, selection: DeviceSelection) -> Self {
            Self {
                list,
                selection,
                list_error: None,
                set_error: None,
                ui_visible: AtomicBool::new(false),
            }
        }

        fn with_list_error(err: DeviceSelectionError) -> Self {
            Self {
                list: AudioDeviceList::default(),
                selection: DeviceSelection::default(),
                list_error: Some(err),
                set_error: None,
                ui_visible: AtomicBool::new(false),
            }
        }

        fn with_set_error(list: AudioDeviceList, err: DeviceSelectionError) -> Self {
            Self {
                list,
                selection: DeviceSelection::default(),
                list_error: None,
                set_error: Some(err),
                ui_visible: AtomicBool::new(false),
            }
        }
    }

    impl DeviceSelectionService for MockDeviceSelectionService {
        fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
            if let Some(err) = &self.list_error {
                return Err(err.clone());
            }
            Ok(self.list.clone())
        }

        fn get_selection(&self) -> DeviceSelection {
            self.selection.clone()
        }

        fn set_selection(
            &self,
            selection: DeviceSelection,
        ) -> Result<DeviceSelection, DeviceSelectionError> {
            if let Some(err) = &self.set_error {
                return Err(err.clone());
            }
            Ok(selection)
        }

        fn set_ui_visible(&self, visible: bool) {
            self.ui_visible.store(visible, Ordering::SeqCst);
        }
    }

    #[test]
    fn list_audio_devices_returns_fixture_from_service() {
        let list = sample_list();
        let service = MockDeviceSelectionService::new(list.clone(), DeviceSelection::default());

        let result = list_audio_devices_impl(&service).expect("list");
        assert_eq!(result.inputs.len(), 2);
        assert_eq!(result.outputs.len(), 2);
        assert_eq!(result, list);
    }

    #[test]
    fn list_audio_devices_maps_internal_error() {
        let service = MockDeviceSelectionService::with_list_error(DeviceSelectionError::internal(
            "enum failed",
        ));

        let err = list_audio_devices_impl(&service).expect_err("internal");
        assert_eq!(err.code, "INTERNAL");
        assert!(!err.message_ja.is_empty());
        assert!(!err.action_ja.is_empty());
    }

    #[test]
    fn get_device_selection_returns_current_selection() {
        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
            None,
        );
        let service = MockDeviceSelectionService::new(sample_list(), selection.clone());

        assert_eq!(get_device_selection_impl(&service), selection);
    }

    #[test]
    fn set_device_selection_maps_invalid_device_error() {
        let service = MockDeviceSelectionService::with_set_error(
            sample_list(),
            DeviceSelectionError::invalid_device(),
        );

        let err = set_device_selection_impl(
            &service,
            DeviceSelection::new(
                Some(AudioDeviceId::new("missing".to_string()).expect("id")),
                None,
            ),
        )
        .expect_err("invalid device");

        assert_eq!(err.code, "INVALID_DEVICE");
        assert_eq!(err.message_ja, "選択したデバイスが見つかりません");
        assert_eq!(err.action_ja, "一覧を更新して別のデバイスを選んでください");
    }

    #[test]
    fn set_device_selection_maps_macos_output_not_default_error() {
        let service = MockDeviceSelectionService::with_set_error(
            sample_list(),
            DeviceSelectionError::macos_output_not_default(),
        );

        let err = set_device_selection_impl(
            &service,
            DeviceSelection::new(
                None,
                Some(AudioDeviceId::new("spk-hdmi".to_string()).expect("id")),
            ),
        )
        .expect_err("macos output");

        assert_eq!(err.code, "MACOS_OUTPUT_NOT_DEFAULT");
        assert!(!err.message_ja.is_empty());
        assert!(!err.action_ja.is_empty());
    }

    #[test]
    fn set_device_selection_success_returns_applied_selection() {
        let service = MockDeviceSelectionService::new(sample_list(), DeviceSelection::default());
        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-default".to_string()).expect("id")),
            Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
        );

        let applied = set_device_selection_impl(&service, selection.clone()).expect("set");
        assert_eq!(applied, selection);
    }

    #[test]
    fn invoke_error_serializes_contract_shape() {
        let err =
            DeviceSelectionInvokeError::from_service_error(DeviceSelectionError::invalid_device());
        assert_invoke_error_serializes_contract_shape(&err, "INVALID_DEVICE");
    }

    #[test]
    fn recording_emitter_captures_selection_changed() {
        let emitter = RecordingDeviceSelectionEventEmitter::new();
        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-default".to_string()).expect("id")),
            None,
        );

        emitter.emit_selection_changed(&selection);

        let events = emitter.selections();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].selection, selection);
        assert!(events[0].timestamp_ms > 0);
    }

    #[test]
    fn recording_emitter_captures_devices_changed() {
        let emitter = RecordingDeviceSelectionEventEmitter::new();
        let list = sample_list();

        emitter.emit_devices_changed(&list, 42_000);

        let events = emitter.devices();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].devices, list);
        assert_eq!(events[0].timestamp_ms, 42_000);
    }

    #[test]
    fn set_audio_device_ui_visible_forwards_to_service() {
        let service = MockDeviceSelectionService::new(sample_list(), DeviceSelection::default());

        set_audio_device_ui_visible_impl(&service, true);
        assert!(service.ui_visible.load(Ordering::SeqCst));

        set_audio_device_ui_visible_impl(&service, false);
        assert!(!service.ui_visible.load(Ordering::SeqCst));
    }
}
