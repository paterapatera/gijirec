use super::super::observability::{
    DeviceSelectionObservability, NoopDeviceSelectionObservability,
    RecordingDeviceSelectionObservability,
};
use super::super::store::DeviceSelectionStore;
use super::*;
use gijirec_domain::audio::fixtures::{mic, sample_device_list};
use gijirec_domain::audio::{
    AudioDeviceId, AudioDeviceList, CaptureError, CapturePhase, DeviceSelection,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
) -> DefaultDeviceSelectionService<N, MockOrchestrator, NoopSpeakerPreflight, MockEvents, MockClock>
{
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

fn service_with_sample_list_events(events: MockEvents, clock: MockClock) -> StandardTestService {
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

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
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
