//! Device listing, selection validation, and capture restart coordination.

mod capture_restart;
mod poll;

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use gijirec_domain::audio::{
    AudioDeviceId, AudioDeviceList, CaptureError, CapturePhase, DeviceSelection,
};

use super::observability::DeviceSelectionObservability;
use super::store::DeviceSelectionStore;

use capture_restart::{device_display_names, selection_id_str, validate_selection};
use poll::{PollShared, new_poll_state, start_poll_thread, stop_poll_thread};

/// Minimum interval between hotplug polls while UI is visible (requirement 5.1).
pub const HOTPLUG_POLL_INTERVAL_MS: u64 = 2_000;

/// Invoke error codes for device selection commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceSelectionErrorCode {
    InvalidDevice,
    MacosOutputNotDefault,
    Internal,
}

impl DeviceSelectionErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidDevice => "INVALID_DEVICE",
            Self::MacosOutputNotDefault => "MACOS_OUTPUT_NOT_DEFAULT",
            Self::Internal => "INTERNAL",
        }
    }
}

/// Application error for device selection operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSelectionError {
    pub code: DeviceSelectionErrorCode,
    pub message_ja: String,
    pub action_ja: String,
}

crate::user_facing_error::impl_message_ja_error_display!(DeviceSelectionError);

impl DeviceSelectionError {
    pub fn invalid_device() -> Self {
        Self {
            code: DeviceSelectionErrorCode::InvalidDevice,
            message_ja: "選択したデバイスが見つかりません".to_string(),
            action_ja: "一覧を更新して別のデバイスを選んでください".to_string(),
        }
    }

    pub fn macos_output_not_default() -> Self {
        Self {
            code: DeviceSelectionErrorCode::MacosOutputNotDefault,
            message_ja: "選択したスピーカーがシステムの出力先になっていません".to_string(),
            action_ja:
                "システム設定 → サウンドで出力先を変更するか、一覧から現在の出力先を選んでください"
                    .to_string(),
        }
    }

    pub fn internal(_detail: impl Into<String>) -> Self {
        Self {
            code: DeviceSelectionErrorCode::Internal,
            message_ja: "デバイス選択の処理に失敗しました".to_string(),
            action_ja: "アプリを再起動してください".to_string(),
        }
    }
}

/// Lists available audio devices (infrastructure adapter implements this).
pub trait DeviceEnumeratorPort: Send + Sync {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError>;
}

/// Capture restart hook for selection changes.
pub trait CaptureSelectionPort: Send {
    fn capture_phase(&self) -> CapturePhase;
    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError>;
}

/// macOS speaker preflight (ADR-0009). Injectable so Windows CI can test the branch.
pub trait SpeakerPreflightPort: Send + Sync {
    fn validate_speaker(
        &self,
        speaker_id: Option<&AudioDeviceId>,
        list: &AudioDeviceList,
    ) -> Result<(), DeviceSelectionError>;
}

/// No-op preflight for non-macOS platforms.
pub struct NoopSpeakerPreflight;

impl SpeakerPreflightPort for NoopSpeakerPreflight {
    fn validate_speaker(
        &self,
        _speaker_id: Option<&AudioDeviceId>,
        _list: &AudioDeviceList,
    ) -> Result<(), DeviceSelectionError> {
        Ok(())
    }
}

/// Enforces macOS default-output rule when enabled (tests set `enabled: true` on any OS).
pub struct MacosSpeakerPreflight {
    pub enabled: bool,
}

impl SpeakerPreflightPort for MacosSpeakerPreflight {
    fn validate_speaker(
        &self,
        speaker_id: Option<&AudioDeviceId>,
        list: &AudioDeviceList,
    ) -> Result<(), DeviceSelectionError> {
        if !self.enabled {
            return Ok(());
        }
        let Some(speaker_id) = speaker_id else {
            return Ok(());
        };
        let Some(device) = list
            .outputs
            .iter()
            .find(|output| output.id().as_str() == speaker_id.as_str())
        else {
            return Err(DeviceSelectionError::invalid_device());
        };
        if device.is_default() {
            Ok(())
        } else {
            Err(DeviceSelectionError::macos_output_not_default())
        }
    }
}

/// Emits selection/list change notifications (presentation layer implements Tauri emit).
pub trait DeviceSelectionEvents: Send + Sync {
    fn emit_selection_changed(&self, selection: &DeviceSelection);
    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64);
}

pub struct NoopDeviceSelectionEvents;

impl DeviceSelectionEvents for NoopDeviceSelectionEvents {
    fn emit_selection_changed(&self, _selection: &DeviceSelection) {}
    fn emit_devices_changed(&self, _devices: &AudioDeviceList, _timestamp_ms: u64) {}
}

/// Clock for hotplug polling (injectable in tests).
pub trait DeviceSelectionClock: Send + Sync {
    fn now_ms(&self) -> u64;

    /// Sleep duration between background hotplug polls (override in tests if needed).
    fn hotplug_poll_interval_ms(&self) -> u64 {
        HOTPLUG_POLL_INTERVAL_MS
    }
}

pub struct SystemClock;

impl DeviceSelectionClock for SystemClock {
    fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Service API per design D-DeviceSelectionService.
pub trait DeviceSelectionService: Send + Sync {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError>;
    fn get_selection(&self) -> DeviceSelection;
    fn set_selection(
        &self,
        selection: DeviceSelection,
    ) -> Result<DeviceSelection, DeviceSelectionError>;
    fn set_ui_visible(&self, visible: bool);
}

/// Default device selection service with injected ports.
pub struct DefaultDeviceSelectionService<E, O, P, Ev, C> {
    store: DeviceSelectionStore,
    pub(crate) shared: Arc<PollShared<E, Ev, C>>,
    orchestrator: Mutex<O>,
    preflight: P,
    observability: Arc<dyn DeviceSelectionObservability>,
    flight: Mutex<()>,
    pub(crate) poll_stop: Arc<AtomicBool>,
    pub(crate) poll_thread: Mutex<Option<JoinHandle<()>>>,
}

impl<E, O, P, Ev, C> DefaultDeviceSelectionService<E, O, P, Ev, C> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: DeviceSelectionStore,
        enumerator: E,
        orchestrator: O,
        preflight: P,
        events: Ev,
        clock: C,
        observability: Arc<dyn DeviceSelectionObservability>,
    ) -> Self {
        Self {
            store,
            shared: Arc::new(PollShared {
                enumerator,
                events,
                clock,
                poll: Mutex::new(new_poll_state()),
            }),
            orchestrator: Mutex::new(orchestrator),
            preflight,
            observability,
            flight: Mutex::new(()),
            poll_stop: Arc::new(AtomicBool::new(false)),
            poll_thread: Mutex::new(None),
        }
    }
}

impl<E, O, P, Ev, C> DefaultDeviceSelectionService<E, O, P, Ev, C>
where
    E: DeviceEnumeratorPort,
    Ev: DeviceSelectionEvents,
    C: DeviceSelectionClock,
{
    /// Drives one hotplug poll tick (for unit tests; production uses the background thread).
    pub fn poll_tick_for_test(&self) -> Result<(), DeviceSelectionError> {
        self.shared.poll_devices_if_due(false)
    }
}

impl<E, O, P, Ev, C> Drop for DefaultDeviceSelectionService<E, O, P, Ev, C> {
    fn drop(&mut self) {
        stop_poll_thread(self);
    }
}

impl<E, O, P, Ev, C> DeviceSelectionService for DefaultDeviceSelectionService<E, O, P, Ev, C>
where
    E: DeviceEnumeratorPort + 'static,
    O: CaptureSelectionPort,
    P: SpeakerPreflightPort,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
{
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
        self.shared.enumerator.list_devices()
    }

    fn get_selection(&self) -> DeviceSelection {
        self.store.get_selection()
    }

    fn set_selection(
        &self,
        selection: DeviceSelection,
    ) -> Result<DeviceSelection, DeviceSelectionError> {
        let _flight = self
            .flight
            .lock()
            .map_err(|_| DeviceSelectionError::internal("flight lock poisoned"))?;

        let list = self.shared.enumerator.list_devices()?;
        validate_selection(&self.preflight, &selection, &list)?;

        if selection == self.store.get_selection() {
            return Ok(selection);
        }

        let (microphone_name, speaker_name) = device_display_names(&list, &selection);
        self.observability
            .log_device_names_debug(microphone_name.as_deref(), speaker_name.as_deref());

        self.store.update(selection.clone());
        self.observability.log_selection_changed(
            selection_id_str(selection.microphone_id()),
            selection_id_str(selection.speaker_id()),
        );
        self.shared
            .events
            .emit_selection_changed(&self.store.get_selection());

        self.restart_capture_until_stable()?;

        Ok(self.store.get_selection())
    }

    fn set_ui_visible(&self, visible: bool) {
        if let Ok(mut poll) = self.shared.poll.lock() {
            poll.ui_visible = visible;
        }
        if visible {
            let _ = self.shared.poll_devices_if_due(true);
            start_poll_thread(self);
        } else {
            stop_poll_thread(self);
        }
    }
}
