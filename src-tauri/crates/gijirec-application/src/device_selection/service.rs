//! Device listing, selection validation, and capture restart coordination.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gijirec_domain::audio::{
    AudioDeviceId, AudioDeviceList, CaptureError, CapturePhase, DeviceSelection,
};

use super::observability::DeviceSelectionObservability;
use super::store::DeviceSelectionStore;

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

struct PollState {
    ui_visible: bool,
    last_poll_ms: u64,
    last_snapshot: Option<AudioDeviceList>,
}

struct PollShared<E, Ev, C> {
    enumerator: E,
    events: Ev,
    clock: C,
    poll: Mutex<PollState>,
}

impl<E, Ev, C> PollShared<E, Ev, C>
where
    E: DeviceEnumeratorPort,
    Ev: DeviceSelectionEvents,
    C: DeviceSelectionClock,
{
    fn poll_devices_if_due(&self, force: bool) -> Result<(), DeviceSelectionError> {
        let mut poll = self
            .poll
            .lock()
            .map_err(|_| DeviceSelectionError::internal("poll lock poisoned"))?;
        if !poll.ui_visible {
            return Ok(());
        }

        let now = self.clock.now_ms();
        if !force && now.saturating_sub(poll.last_poll_ms) < HOTPLUG_POLL_INTERVAL_MS {
            return Ok(());
        }

        let devices = self.enumerator.list_devices()?;
        poll.last_poll_ms = now;

        let changed = poll.last_snapshot.as_ref() != Some(&devices);
        if changed {
            poll.last_snapshot = Some(devices.clone());
            self.events.emit_devices_changed(&devices, now);
        }
        Ok(())
    }
}

/// Default device selection service with injected ports.
pub struct DefaultDeviceSelectionService<E, O, P, Ev, C> {
    store: DeviceSelectionStore,
    shared: Arc<PollShared<E, Ev, C>>,
    orchestrator: Mutex<O>,
    preflight: P,
    observability: Arc<dyn DeviceSelectionObservability>,
    flight: Mutex<()>,
    poll_stop: Arc<AtomicBool>,
    poll_thread: Mutex<Option<JoinHandle<()>>>,
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
                poll: Mutex::new(PollState {
                    ui_visible: false,
                    last_poll_ms: 0,
                    last_snapshot: None,
                }),
            }),
            orchestrator: Mutex::new(orchestrator),
            preflight,
            observability,
            flight: Mutex::new(()),
            poll_stop: Arc::new(AtomicBool::new(false)),
            poll_thread: Mutex::new(None),
        }
    }

    fn stop_poll_thread(&self) {
        self.poll_stop.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.poll_thread.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
        self.poll_stop.store(false, Ordering::Relaxed);
    }
}

impl<E, O, P, Ev, C> Drop for DefaultDeviceSelectionService<E, O, P, Ev, C> {
    fn drop(&mut self) {
        self.stop_poll_thread();
    }
}

fn run_hotplug_poll_loop<E, Ev, C>(shared: Arc<PollShared<E, Ev, C>>, stop: Arc<AtomicBool>)
where
    E: DeviceEnumeratorPort + 'static,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
{
    loop {
        let interval_ms = shared.clock.hotplug_poll_interval_ms();
        sleep_poll_interval(&stop, interval_ms);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let ui_visible = shared
            .poll
            .lock()
            .map(|poll| poll.ui_visible)
            .unwrap_or(false);
        if !ui_visible {
            break;
        }
        let _ = shared.poll_devices_if_due(false);
    }
}

fn sleep_poll_interval(stop: &AtomicBool, interval_ms: u64) {
    const CHUNK_MS: u64 = 50;
    let mut elapsed = 0;
    while elapsed < interval_ms && !stop.load(Ordering::Relaxed) {
        let step = CHUNK_MS.min(interval_ms - elapsed);
        thread::sleep(Duration::from_millis(step));
        elapsed += step;
    }
}

/* jscpd:ignore-start */
impl<E, O, P, Ev, C> DefaultDeviceSelectionService<E, O, P, Ev, C>
where
    E: DeviceEnumeratorPort + 'static,
    O: CaptureSelectionPort,
    P: SpeakerPreflightPort,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
    /* jscpd:ignore-end */
{
    /// Drives one hotplug poll tick (for unit tests; production uses the background thread).
    pub fn poll_tick_for_test(&self) -> Result<(), DeviceSelectionError> {
        self.shared.poll_devices_if_due(false)
    }

    fn start_poll_thread(&self) {
        self.stop_poll_thread();
        self.poll_stop.store(false, Ordering::Relaxed);
        let shared = Arc::clone(&self.shared);
        let stop = Arc::clone(&self.poll_stop);
        let handle = thread::spawn(move || run_hotplug_poll_loop(shared, stop));
        if let Ok(mut guard) = self.poll_thread.lock() {
            *guard = Some(handle);
        }
    }

    fn validate_selection(
        &self,
        selection: &DeviceSelection,
        list: &AudioDeviceList,
    ) -> Result<(), DeviceSelectionError> {
        if let Some(mic_id) = selection.microphone_id() {
            let exists = list
                .inputs
                .iter()
                .any(|device| device.id().as_str() == mic_id.as_str());
            if !exists {
                return Err(DeviceSelectionError::invalid_device());
            }
        }

        if let Some(speaker_id) = selection.speaker_id() {
            let exists = list
                .outputs
                .iter()
                .any(|device| device.id().as_str() == speaker_id.as_str());
            if !exists {
                return Err(DeviceSelectionError::invalid_device());
            }
        }

        self.preflight
            .validate_speaker(selection.speaker_id(), list)
    }

    fn should_restart_capture(phase: CapturePhase) -> bool {
        matches!(
            phase,
            CapturePhase::Capturing | CapturePhase::Starting | CapturePhase::Error
        )
    }

    fn selection_id_str(id: Option<&AudioDeviceId>) -> Option<&str> {
        id.map(|device_id| device_id.as_str())
    }

    fn device_display_names(
        list: &AudioDeviceList,
        selection: &DeviceSelection,
    ) -> (Option<String>, Option<String>) {
        let microphone_name = selection.microphone_id().and_then(|id| {
            list.inputs
                .iter()
                .find(|device| device.id().as_str() == id.as_str())
                .map(|device| device.name().to_string())
        });
        let speaker_name = selection.speaker_id().and_then(|id| {
            list.outputs
                .iter()
                .find(|device| device.id().as_str() == id.as_str())
                .map(|device| device.name().to_string())
        });
        (microphone_name, speaker_name)
    }

    fn restart_capture_until_stable(&self) -> Result<(), DeviceSelectionError> {
        loop {
            let selection = self.store.get_selection();
            let microphone_id = Self::selection_id_str(selection.microphone_id());
            let speaker_id = Self::selection_id_str(selection.speaker_id());
            if !self.restart_capture_once(&selection, microphone_id, speaker_id)? {
                return Ok(());
            }
            if self.store.get_selection() == selection {
                break;
            }
        }
        Ok(())
    }

    /// Returns `false` when capture is not in a restart-eligible phase (caller should stop).
    fn restart_capture_once(
        &self,
        selection: &DeviceSelection,
        microphone_id: Option<&str>,
        speaker_id: Option<&str>,
    ) -> Result<bool, DeviceSelectionError> {
        let mut orchestrator = self
            .orchestrator
            .lock()
            .map_err(|_| DeviceSelectionError::internal("orchestrator lock poisoned"))?;
        if !Self::should_restart_capture(orchestrator.capture_phase()) {
            return Ok(false);
        }

        let correlation_id = uuid::Uuid::new_v4().to_string();
        let started_ms = self.shared.clock.now_ms();
        self.observability
            .log_recapture_started(&correlation_id, microphone_id, speaker_id);

        orchestrator
            .restart_with_selection(selection)
            .map_err(|err| DeviceSelectionError::internal(err.to_string()))?;

        let duration_ms = self.shared.clock.now_ms().saturating_sub(started_ms);
        self.observability
            .log_recapture_completed(&correlation_id, duration_ms);
        Ok(true)
    }
}

/* jscpd:ignore-start */
impl<E, O, P, Ev, C> DeviceSelectionService for DefaultDeviceSelectionService<E, O, P, Ev, C>
where
    E: DeviceEnumeratorPort + 'static,
    O: CaptureSelectionPort,
    P: SpeakerPreflightPort,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
    /* jscpd:ignore-end */
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
        self.validate_selection(&selection, &list)?;

        if selection == self.store.get_selection() {
            return Ok(selection);
        }

        let (microphone_name, speaker_name) = Self::device_display_names(&list, &selection);
        self.observability
            .log_device_names_debug(microphone_name.as_deref(), speaker_name.as_deref());

        self.store.update(selection.clone());
        self.observability.log_selection_changed(
            Self::selection_id_str(selection.microphone_id()),
            Self::selection_id_str(selection.speaker_id()),
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
            self.start_poll_thread();
        } else {
            self.stop_poll_thread();
        }
    }
}

#[cfg(test)]
#[allow(clippy::type_complexity)]
mod tests {
    use super::super::observability::{
        NoopDeviceSelectionObservability, RecordingDeviceSelectionObservability,
    };
    use super::*;
    use gijirec_domain::audio::fixtures::{mic, sample_device_list};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn sample_list() -> AudioDeviceList {
        sample_device_list()
    }

    fn mock_orchestrator(phase: CapturePhase) -> MockOrchestrator {
        MockOrchestrator {
            phase,
            restarts: Arc::new(Mutex::new(Vec::new())),
            on_restart: None,
        }
    }

    fn disabled_macos_preflight() -> MacosSpeakerPreflight {
        MacosSpeakerPreflight { enabled: false }
    }

    fn mock_orchestrator_with_restarts(
        phase: CapturePhase,
        restarts: Arc<Mutex<Vec<DeviceSelection>>>,
    ) -> MockOrchestrator {
        MockOrchestrator {
            phase,
            restarts,
            on_restart: None,
        }
    }

    type StandardTestService = DefaultDeviceSelectionService<
        MockEnumerator,
        MockOrchestrator,
        NoopSpeakerPreflight,
        MockEvents,
        MockClock,
    >;

    type EmptyListTestService = DefaultDeviceSelectionService<
        MockEnumerator,
        MockOrchestrator,
        NoopSpeakerPreflight,
        NoopDeviceSelectionEvents,
        MockClock,
    >;

    fn mock_event_buffers() -> (
        MockEvents,
        Arc<Mutex<Vec<DeviceSelection>>>,
        Arc<Mutex<Vec<AudioDeviceList>>>,
    ) {
        let selections = Arc::new(Mutex::new(Vec::new()));
        let device_changes = Arc::new(Mutex::new(Vec::new()));
        let events = MockEvents {
            selections: Arc::clone(&selections),
            device_changes: Arc::clone(&device_changes),
        };
        (events, selections, device_changes)
    }

    fn build_idle_device_service<N>(
        enumerator: N,
        events: MockEvents,
        clock: MockClock,
    ) -> DefaultDeviceSelectionService<
        N,
        MockOrchestrator,
        NoopSpeakerPreflight,
        MockEvents,
        MockClock,
    > {
        DefaultDeviceSelectionService::new(
            DeviceSelectionStore::new(),
            enumerator,
            mock_orchestrator(CapturePhase::Idle),
            NoopSpeakerPreflight,
            events,
            clock,
            Arc::new(NoopDeviceSelectionObservability),
        )
    }

    fn service_with_sample_list_events(
        events: MockEvents,
        clock: MockClock,
    ) -> StandardTestService {
        build_idle_device_service(
            MockEnumerator {
                list: sample_list(),
            },
            events,
            clock,
        )
    }

    fn empty_list_service() -> EmptyListTestService {
        DefaultDeviceSelectionService::new(
            DeviceSelectionStore::new(),
            MockEnumerator {
                list: AudioDeviceList::default(),
            },
            mock_orchestrator(CapturePhase::Idle),
            NoopSpeakerPreflight,
            NoopDeviceSelectionEvents,
            MockClock::new(0),
            Arc::new(NoopDeviceSelectionObservability),
        )
    }

    fn missing_mic_id() -> AudioDeviceId {
        AudioDeviceId::new("missing-mic".to_string()).expect("id")
    }

    fn missing_speaker_id() -> AudioDeviceId {
        AudioDeviceId::new("missing-spk".to_string()).expect("id")
    }

    fn expect_invalid_device<P: SpeakerPreflightPort>(
        service: &DefaultDeviceSelectionService<
            MockEnumerator,
            MockOrchestrator,
            P,
            MockEvents,
            MockClock,
        >,
        selection: DeviceSelection,
        label: &'static str,
    ) {
        let err = service.set_selection(selection).expect_err(label);
        assert_eq!(err.code, DeviceSelectionErrorCode::InvalidDevice);
    }

    fn default_valid_selection() -> DeviceSelection {
        DeviceSelection::new(
            Some(AudioDeviceId::new("mic-default".to_string()).expect("id")),
            Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
        )
    }

    fn default_and_usb_selections() -> (DeviceSelection, DeviceSelection) {
        let sel1 = default_valid_selection();
        let sel2 = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
            Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
        );
        (sel1, sel2)
    }

    type HotplugVisibleService = DefaultDeviceSelectionService<
        MutableMockEnumerator,
        MockOrchestrator,
        NoopSpeakerPreflight,
        MockEvents,
        MockClock,
    >;

    type HotplugVisibleFixture = (
        HotplugVisibleService,
        Arc<Mutex<Vec<AudioDeviceList>>>,
        Arc<Mutex<AudioDeviceList>>,
        MockClock,
    );

    fn hotplug_visible_service() -> HotplugVisibleFixture {
        let list = Arc::new(Mutex::new(sample_list()));
        let clock = MockClock::new(1_000);
        let (service, device_changes) = hotplug_test_service(Arc::clone(&list), clock.clone());
        (service, device_changes, list, clock)
    }

    fn advance_hotplug_and_push_mic(
        list: &Arc<Mutex<AudioDeviceList>>,
        clock: &MockClock,
        mic_id: &str,
    ) {
        clock.advance(HOTPLUG_POLL_INTERVAL_MS);
        list.lock().expect("lock").inputs.push(mic(mic_id, false));
    }

    fn visible_hotplug_with_initial_emit() -> HotplugVisibleFixture {
        let (service, device_changes, list, clock) = hotplug_visible_service();
        service.set_ui_visible(true);
        assert_eq!(device_changes.lock().expect("lock").len(), 1);
        (service, device_changes, list, clock)
    }

    fn capturing_service_with_restarts(
        restarts: Arc<Mutex<Vec<DeviceSelection>>>,
    ) -> (
        DefaultDeviceSelectionService<
            MockEnumerator,
            MockOrchestrator,
            MacosSpeakerPreflight,
            MockEvents,
            MockClock,
        >,
        Arc<Mutex<Vec<DeviceSelection>>>,
    ) {
        service_with_orchestrator_noop(
            mock_orchestrator_with_restarts(CapturePhase::Capturing, restarts),
            disabled_macos_preflight(),
        )
    }

    struct MockEnumerator {
        list: AudioDeviceList,
    }

    impl DeviceEnumeratorPort for MockEnumerator {
        fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
            Ok(self.list.clone())
        }
    }

    struct MockOrchestrator {
        phase: CapturePhase,
        restarts: Arc<Mutex<Vec<DeviceSelection>>>,
        on_restart: Option<Arc<dyn Fn() + Send + Sync>>,
    }

    impl CaptureSelectionPort for MockOrchestrator {
        fn capture_phase(&self) -> CapturePhase {
            self.phase
        }

        fn restart_with_selection(
            &mut self,
            selection: &DeviceSelection,
        ) -> Result<(), CaptureError> {
            self.restarts.lock().expect("lock").push(selection.clone());
            if let Some(hook) = &self.on_restart {
                hook();
            }
            Ok(())
        }
    }

    struct MockEvents {
        selections: Arc<Mutex<Vec<DeviceSelection>>>,
        device_changes: Arc<Mutex<Vec<AudioDeviceList>>>,
    }

    impl DeviceSelectionEvents for MockEvents {
        fn emit_selection_changed(&self, selection: &DeviceSelection) {
            self.selections
                .lock()
                .expect("lock")
                .push(selection.clone());
        }

        fn emit_devices_changed(&self, devices: &AudioDeviceList, _timestamp_ms: u64) {
            self.device_changes
                .lock()
                .expect("lock")
                .push(devices.clone());
        }
    }

    #[derive(Clone)]
    struct MockClock {
        now: Arc<AtomicU64>,
    }

    impl MockClock {
        fn new(initial_ms: u64) -> Self {
            Self {
                now: Arc::new(AtomicU64::new(initial_ms)),
            }
        }

        fn advance(&self, ms: u64) {
            self.now.fetch_add(ms, Ordering::SeqCst);
        }
    }

    impl DeviceSelectionClock for MockClock {
        fn now_ms(&self) -> u64 {
            self.now.load(Ordering::SeqCst)
        }
    }

    struct MutableMockEnumerator {
        list: Arc<Mutex<AudioDeviceList>>,
    }

    impl DeviceEnumeratorPort for MutableMockEnumerator {
        fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
            Ok(self.list.lock().expect("lock").clone())
        }
    }

    fn hotplug_test_service(
        list: Arc<Mutex<AudioDeviceList>>,
        clock: MockClock,
    ) -> (
        DefaultDeviceSelectionService<
            MutableMockEnumerator,
            MockOrchestrator,
            NoopSpeakerPreflight,
            MockEvents,
            MockClock,
        >,
        Arc<Mutex<Vec<AudioDeviceList>>>,
    ) {
        let (events, _, device_changes) = mock_event_buffers();
        let service = build_idle_device_service(
            MutableMockEnumerator {
                list: Arc::clone(&list),
            },
            events,
            clock,
        );
        (service, device_changes)
    }

    fn service_with_orchestrator(
        orchestrator: MockOrchestrator,
        preflight: MacosSpeakerPreflight,
        observability: Arc<dyn DeviceSelectionObservability>,
    ) -> (
        DefaultDeviceSelectionService<
            MockEnumerator,
            MockOrchestrator,
            MacosSpeakerPreflight,
            MockEvents,
            MockClock,
        >,
        Arc<Mutex<Vec<DeviceSelection>>>,
    ) {
        let restarts = Arc::clone(&orchestrator.restarts);
        let (events, selections, device_changes) = mock_event_buffers();
        let _ = (selections, device_changes);
        let service = DefaultDeviceSelectionService::new(
            DeviceSelectionStore::new(),
            MockEnumerator {
                list: sample_list(),
            },
            orchestrator,
            preflight,
            events,
            MockClock::new(0),
            observability,
        );
        (service, restarts)
    }

    fn service_with_orchestrator_noop(
        orchestrator: MockOrchestrator,
        preflight: MacosSpeakerPreflight,
    ) -> (
        DefaultDeviceSelectionService<
            MockEnumerator,
            MockOrchestrator,
            MacosSpeakerPreflight,
            MockEvents,
            MockClock,
        >,
        Arc<Mutex<Vec<DeviceSelection>>>,
    ) {
        service_with_orchestrator(
            orchestrator,
            preflight,
            Arc::new(NoopDeviceSelectionObservability),
        )
    }

    #[test]
    fn list_devices_returns_enumerator_list_including_empty() {
        let (service, _) = service_with_orchestrator_noop(
            mock_orchestrator(CapturePhase::Idle),
            disabled_macos_preflight(),
        );

        let list = service.list_devices().expect("list");
        assert_eq!(list.inputs.len(), 2);
        assert_eq!(list.outputs.len(), 2);

        let empty_service = empty_list_service();
        assert!(
            empty_service
                .list_devices()
                .expect("list")
                .inputs
                .is_empty()
        );
    }

    /// Design unit test 1: unknown input ID → INVALID_DEVICE (no silent fallback).
    #[test]
    fn set_selection_rejects_unknown_device_with_invalid_device() {
        let (service, _) = service_with_orchestrator_noop(
            mock_orchestrator(CapturePhase::Capturing),
            disabled_macos_preflight(),
        );

        expect_invalid_device(
            &service,
            DeviceSelection::new(Some(missing_mic_id()), None),
            "invalid mic",
        );
    }

    /// Design unit test 1: unknown output ID → INVALID_DEVICE (no silent fallback).
    #[test]
    fn set_selection_rejects_unknown_speaker_with_invalid_device() {
        let (service, _) = service_with_orchestrator_noop(
            mock_orchestrator(CapturePhase::Capturing),
            disabled_macos_preflight(),
        );

        expect_invalid_device(
            &service,
            DeviceSelection::new(None, Some(missing_speaker_id())),
            "invalid speaker",
        );
    }

    /// Requirement 4.5: validation failure must not corrupt stored selection or trigger restart.
    #[test]
    fn set_selection_invalid_device_preserves_prior_selection() {
        let restarts = Arc::new(Mutex::new(Vec::new()));
        let (service, restart_log) = capturing_service_with_restarts(Arc::clone(&restarts));

        let valid = default_valid_selection();
        service.set_selection(valid.clone()).expect("valid");
        assert_eq!(restart_log.lock().expect("lock").len(), 1);

        expect_invalid_device(
            &service,
            DeviceSelection::new(Some(missing_mic_id()), None),
            "invalid mic",
        );
        assert_eq!(
            service.get_selection(),
            valid,
            "store must remain unchanged after INVALID_DEVICE"
        );
        assert_eq!(
            restart_log.lock().expect("lock").len(),
            1,
            "invalid selection must not trigger capture restart"
        );
    }

    /// Design unit test 2: identical consecutive selection is a no-op (no second restart).
    #[test]
    fn set_selection_is_idempotent_without_restart() {
        let restarts = Arc::new(Mutex::new(Vec::new()));
        let (service, restart_log) = capturing_service_with_restarts(Arc::clone(&restarts));

        let selection = default_valid_selection();

        service.set_selection(selection.clone()).expect("first");
        assert_eq!(restart_log.lock().expect("lock").len(), 1);

        service.set_selection(selection).expect("second");
        assert_eq!(
            restart_log.lock().expect("lock").len(),
            1,
            "identical selection must not restart again"
        );
    }

    #[test]
    fn set_selection_restarts_from_error_phase() {
        let (service, restart_log) = service_with_orchestrator_noop(
            mock_orchestrator(CapturePhase::Error),
            disabled_macos_preflight(),
        );

        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
            None,
        );
        service.set_selection(selection.clone()).expect("recover");

        let restarts = restart_log.lock().expect("lock");
        assert_eq!(restarts.len(), 1);
        assert_eq!(restarts[0], selection);
    }

    /// Design unit test 4: macOS preflight rejects non-default speaker (MACOS_OUTPUT_NOT_DEFAULT).
    #[test]
    fn macos_preflight_rejects_non_default_speaker_when_enabled() {
        let (service, _) = service_with_orchestrator_noop(
            mock_orchestrator(CapturePhase::Idle),
            MacosSpeakerPreflight { enabled: true },
        );

        let err = service
            .set_selection(DeviceSelection::new(
                None,
                Some(AudioDeviceId::new("spk-hdmi".to_string()).expect("id")),
            ))
            .expect_err("non-default speaker");
        assert_eq!(err.code, DeviceSelectionErrorCode::MacosOutputNotDefault);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_cfg_default_speaker_preflight_is_available() {
        let preflight = MacosSpeakerPreflight { enabled: true };
        let list = sample_list();
        let default = AudioDeviceId::new("spk-default".to_string()).expect("id");
        assert!(preflight.validate_speaker(Some(&default), &list).is_ok());
    }

    /// Design unit test 3: flight mutex serializes concurrent changes; final selection wins.
    #[test]
    fn set_selection_serializes_concurrent_changes_during_slow_restart() {
        use std::sync::Condvar;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::thread;

        let restarts = Arc::new(Mutex::new(Vec::new()));
        let restart_count = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let gate_hook = Arc::clone(&gate);
        let restart_count_hook = Arc::clone(&restart_count);

        let (service, restart_log) = service_with_orchestrator_noop(
            MockOrchestrator {
                phase: CapturePhase::Capturing,
                restarts: Arc::clone(&restarts),
                on_restart: Some(Arc::new(move || {
                    let n = restart_count_hook.fetch_add(1, Ordering::SeqCst);
                    if n > 0 {
                        return;
                    }
                    let (lock, cvar) = &*gate_hook;
                    let mut blocked = lock.lock().expect("lock");
                    *blocked = true;
                    cvar.notify_all();
                    while *blocked {
                        blocked = cvar.wait(blocked).expect("wait");
                    }
                })),
            },
            disabled_macos_preflight(),
        );
        let service = Arc::new(service);

        let (sel1, sel2) = default_and_usb_selections();

        let svc_first = Arc::clone(&service);
        let sel1_for_thread = sel1.clone();
        let first = thread::spawn(move || svc_first.set_selection(sel1_for_thread));

        {
            let (lock, cvar) = &*gate;
            let mut entered = lock.lock().expect("lock");
            while !*entered {
                entered = cvar.wait(entered).expect("wait");
            }
        }
        assert_eq!(restart_count.load(Ordering::SeqCst), 1);

        let svc_second = Arc::clone(&service);
        let sel2_for_thread = sel2.clone();
        let second = thread::spawn(move || svc_second.set_selection(sel2_for_thread));

        thread::sleep(Duration::from_millis(50));
        assert!(
            !second.is_finished(),
            "second set_selection must wait while first restart holds flight lock"
        );

        {
            let (lock, cvar) = &*gate;
            let mut entered = lock.lock().expect("lock");
            *entered = false;
            cvar.notify_all();
        }

        first.join().expect("join first").expect("first selection");
        second
            .join()
            .expect("join second")
            .expect("second selection");

        let log = restart_log.lock().expect("lock");
        assert_eq!(log.len(), 2);
        assert_eq!(log.last().expect("last restart"), &sel2);
        assert_eq!(service.get_selection(), sel2);
    }

    /// Design unit test 3 (sequential): rapid changes apply in order; latest selection is stored.
    #[test]
    fn sequential_selection_changes_restart_with_latest() {
        let (service, restart_log) = service_with_orchestrator_noop(
            mock_orchestrator(CapturePhase::Capturing),
            disabled_macos_preflight(),
        );

        let (sel1, sel2) = default_and_usb_selections();

        service.set_selection(sel1).expect("first");
        service.set_selection(sel2.clone()).expect("second");

        let log = restart_log.lock().expect("lock");
        assert_eq!(log.len(), 2);
        assert_eq!(log.last().expect("last"), &sel2);
        assert_eq!(service.get_selection(), sel2);
    }

    #[test]
    fn set_ui_visible_emits_devices_changed_on_first_poll() {
        let (events, _, device_changes) = mock_event_buffers();
        let service = service_with_sample_list_events(events, MockClock::new(1_000));

        service.set_ui_visible(true);
        assert_eq!(device_changes.lock().expect("lock").len(), 1);

        service.poll_tick_for_test().expect("tick");
        assert_eq!(
            device_changes.lock().expect("lock").len(),
            1,
            "poll within 2s must not emit again"
        );

        service.set_ui_visible(false);
    }

    #[test]
    fn ui_visible_false_poll_tick_does_not_emit() {
        let list = Arc::new(Mutex::new(sample_list()));
        let clock = MockClock::new(0);
        let (service, device_changes) = hotplug_test_service(Arc::clone(&list), clock);

        service.poll_tick_for_test().expect("tick");
        assert_eq!(device_changes.lock().expect("lock").len(), 0);
    }

    #[test]
    fn hotplug_emit_after_interval_when_list_changes() {
        let (service, device_changes, list, clock) = visible_hotplug_with_initial_emit();

        advance_hotplug_and_push_mic(&list, &clock, "mic-new");

        service.poll_tick_for_test().expect("tick");
        assert_eq!(
            device_changes.lock().expect("lock").len(),
            2,
            "list change after interval must emit devices-changed"
        );

        service.set_ui_visible(false);
    }

    #[test]
    fn set_selection_emits_observability_ids_and_restart_duration() {
        let obs = RecordingDeviceSelectionObservability::new();
        let obs_for_service = obs.clone();
        let (service, _) = service_with_orchestrator(
            mock_orchestrator(CapturePhase::Capturing),
            disabled_macos_preflight(),
            Arc::new(obs_for_service),
        );

        let selection = DeviceSelection::new(
            Some(AudioDeviceId::new("mic-usb".to_string()).expect("id")),
            Some(AudioDeviceId::new("spk-default".to_string()).expect("id")),
        );
        service.set_selection(selection).expect("set");

        let changed = obs.selection_changed.lock().expect("lock");
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0.as_deref(), Some("mic-usb"));
        assert_eq!(changed[0].1.as_deref(), Some("spk-default"));

        let started = obs.recapture_started.lock().expect("lock");
        assert_eq!(started.len(), 1);
        assert!(!started[0].0.is_empty(), "correlation_id required");
        assert_eq!(started[0].1.as_deref(), Some("mic-usb"));
        assert_eq!(started[0].2.as_deref(), Some("spk-default"));

        let completed = obs.recapture_completed.lock().expect("lock");
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].0, started[0].0);

        let debug_names = obs.device_names_debug.lock().expect("lock");
        assert_eq!(debug_names.len(), 1);
        assert_eq!(debug_names[0].0.as_deref(), Some("Mic mic-usb"));
        assert_eq!(debug_names[0].1.as_deref(), Some("Speaker spk-default"));
    }

    #[test]
    fn hotplug_no_emit_after_ui_hidden() {
        let (service, device_changes, list, clock) = visible_hotplug_with_initial_emit();

        service.set_ui_visible(false);

        advance_hotplug_and_push_mic(&list, &clock, "mic-new");

        service.poll_tick_for_test().expect("tick");
        assert_eq!(
            device_changes.lock().expect("lock").len(),
            1,
            "hidden UI must not emit on tick"
        );
    }
}
