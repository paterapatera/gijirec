//! Capture session state machine (`docs/contracts/capture-session-toggle.md`).

use std::sync::{Arc, Mutex};

use gijirec_domain::audio::CapturePhase;
use gijirec_domain::capture_session::{CaptureSessionErrorCode, CaptureSessionPhase};

use crate::capture::orchestrator::CaptureOrchestrator;
use crate::device_selection::DeviceSelectionService;

use super::CaptureSessionObservability;

/// Read model aligned with contract `CaptureSessionState` (capture_phase as domain enum).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSessionSnapshot {
    pub session_phase: CaptureSessionPhase,
    pub transition_busy: bool,
    pub capture_phase: CapturePhase,
    pub timestamp_ms: u64,
}

/// User-facing error for `start_capture_session`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSessionError {
    pub code: CaptureSessionErrorCode,
    pub message_ja: String,
    pub action_ja: String,
}

crate::user_facing_error::impl_message_ja_error_display!(CaptureSessionError);

impl CaptureSessionError {
    pub fn transition_busy() -> Self {
        Self {
            code: CaptureSessionErrorCode::TransitionBusy,
            message_ja: "キャプチャセッションの開始を処理中です。".to_string(),
            action_ja: "処理が完了するまでお待ちください。".to_string(),
        }
    }

    pub fn capture_start_failed() -> Self {
        Self {
            code: CaptureSessionErrorCode::CaptureStartFailed,
            message_ja: "会議音声の取り込みを開始できませんでした。".to_string(),
            action_ja: "デバイス設定を確認して、もう一度お試しください。".to_string(),
        }
    }

    pub fn unsupported_platform() -> Self {
        Self {
            code: CaptureSessionErrorCode::UnsupportedPlatform,
            message_ja:
                "gijirec Audio Capture は Linux をサポートしていません。Windows または macOS でご利用ください。"
                    .to_string(),
            action_ja: "サポートされている OS でご利用ください。".to_string(),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        let _ = detail.into();
        Self {
            code: CaptureSessionErrorCode::Internal,
            message_ja: "キャプチャセッションの処理に失敗しました。".to_string(),
            action_ja: "アプリを再起動してください。".to_string(),
        }
    }
}

/// Injectable platform probe (mirrors presentation `CapturePlatformSupport`).
pub trait CaptureSessionPlatform: Send + Sync {
    fn is_capture_supported(&self) -> bool;
}

/// Processing lifecycle hooks for capture ingest (mirrors presentation hook surface).
pub trait CaptureSessionProcessingHook: Send + Sync {
    fn on_capture_started(&self);
    fn on_capture_stopping(&self);
}

pub struct NoopCaptureSessionProcessingHook;

impl CaptureSessionProcessingHook for NoopCaptureSessionProcessingHook {
    fn on_capture_started(&self) {}
    fn on_capture_stopping(&self) {}
}

/// Emits `capture-session://state-changed` (presentation wires Tauri).
pub trait CaptureSessionEvents: Send + Sync {
    fn emit_state_changed(&self, state: &CaptureSessionSnapshot);
}

pub struct NoopCaptureSessionEvents;

impl CaptureSessionEvents for NoopCaptureSessionEvents {
    fn emit_state_changed(&self, _state: &CaptureSessionSnapshot) {}
}

pub trait CaptureSessionClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub struct SystemCaptureSessionClock;

/* jscpd:ignore-start — same wall-clock helper as DeviceSelectionClock */
impl CaptureSessionClock for SystemCaptureSessionClock {
    fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}
/* jscpd:ignore-end */

/// Application API per design D-CaptureSessionService.
pub trait CaptureSessionServiceApi: Send + Sync {
    fn get_state(&self) -> CaptureSessionSnapshot;
    fn start(&self) -> Result<CaptureSessionSnapshot, CaptureSessionError>;
}

struct SessionPhaseState {
    session_phase: CaptureSessionPhase,
}

struct SessionShared {
    phase: Mutex<SessionPhaseState>,
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    events: Arc<dyn CaptureSessionEvents>,
    clock: Arc<dyn CaptureSessionClock>,
}

fn transition_busy_for(phase: CaptureSessionPhase) -> bool {
    phase == CaptureSessionPhase::Starting
}

fn build_snapshot(
    session_phase: CaptureSessionPhase,
    capture_phase: CapturePhase,
    timestamp_ms: u64,
) -> CaptureSessionSnapshot {
    CaptureSessionSnapshot {
        session_phase,
        transition_busy: transition_busy_for(session_phase),
        capture_phase,
        timestamp_ms,
    }
}

fn snapshot_from_parts(shared: &SessionShared) -> CaptureSessionSnapshot {
    let session_phase = shared
        .phase
        .lock()
        .expect("session phase lock")
        .session_phase;
    let capture_phase = shared
        .orchestrator
        .lock()
        .expect("orchestrator lock")
        .phase();
    build_snapshot(session_phase, capture_phase, shared.clock.now_ms())
}

/// Session facade over capture orchestration (start-only).
pub struct CaptureSessionService {
    shared: Arc<SessionShared>,
    device_selection: Arc<dyn DeviceSelectionService>,
    platform: Arc<dyn CaptureSessionPlatform>,
    processing: Arc<dyn CaptureSessionProcessingHook>,
    observability: Arc<dyn CaptureSessionObservability>,
}

impl CaptureSessionService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
        device_selection: Arc<dyn DeviceSelectionService>,
        platform: Arc<dyn CaptureSessionPlatform>,
        processing: Arc<dyn CaptureSessionProcessingHook>,
        events: Arc<dyn CaptureSessionEvents>,
        clock: Arc<dyn CaptureSessionClock>,
        observability: Arc<dyn CaptureSessionObservability>,
    ) -> Self {
        let shared = Arc::new(SessionShared {
            phase: Mutex::new(SessionPhaseState {
                session_phase: CaptureSessionPhase::Idle,
            }),
            orchestrator,
            events,
            clock,
        });
        Self {
            shared,
            device_selection,
            platform,
            processing,
            observability,
        }
    }

    fn set_session_phase(&self, next: CaptureSessionPhase) {
        let prev = self.session_phase();
        if prev != next {
            self.observability
                .log_session_phase_transition(prev, next, transition_busy_for(next));
        }
        self.shared
            .phase
            .lock()
            .expect("session phase lock")
            .session_phase = next;
    }

    fn session_phase(&self) -> CaptureSessionPhase {
        self.shared
            .phase
            .lock()
            .expect("session phase lock")
            .session_phase
    }

    fn emit_state(&self) {
        let snapshot = self.get_state();
        self.shared.events.emit_state_changed(&snapshot);
    }

    fn reject_if_transition_busy(
        &self,
        phase: CaptureSessionPhase,
    ) -> Result<(), CaptureSessionError> {
        if transition_busy_for(phase) {
            return Err(CaptureSessionError::transition_busy());
        }
        Ok(())
    }

    fn begin_start(&self) -> Result<(), CaptureSessionError> {
        let phase = self.session_phase();
        if phase == CaptureSessionPhase::Active {
            return Ok(());
        }
        self.reject_if_transition_busy(phase)?;
        if !self.platform.is_capture_supported() {
            return Err(CaptureSessionError::unsupported_platform());
        }
        self.set_session_phase(CaptureSessionPhase::Starting);
        self.emit_state();
        Ok(())
    }

    fn complete_start(&self) -> Result<CaptureSessionSnapshot, CaptureSessionError> {
        let selection = self.device_selection.get_selection();
        let start_result = {
            let mut orch = self
                .shared
                .orchestrator
                .lock()
                .map_err(|_| CaptureSessionError::internal("orchestrator lock poisoned"))?;
            orch.start_with_selection(&selection)
        };

        match start_result {
            Ok(()) => {
                self.processing.on_capture_started();
                self.set_session_phase(CaptureSessionPhase::Active);
                self.emit_state();
                Ok(self.get_state())
            }
            Err(_) => {
                let _ = self.shared.orchestrator.lock().map(|mut orch| orch.stop());
                self.set_session_phase(CaptureSessionPhase::Idle);
                self.emit_state();
                Err(CaptureSessionError::capture_start_failed())
            }
        }
    }
}

impl CaptureSessionServiceApi for CaptureSessionService {
    fn get_state(&self) -> CaptureSessionSnapshot {
        snapshot_from_parts(&self.shared)
    }

    fn start(&self) -> Result<CaptureSessionSnapshot, CaptureSessionError> {
        self.begin_start()?;
        if self.session_phase() == CaptureSessionPhase::Active {
            return Ok(self.get_state());
        }
        self.complete_start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture_session::{
        NoopCaptureSessionObservability, RecordingCaptureSessionObservability,
    };
    use gijirec_domain::audio::{CaptureError, DeviceSelection};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("condition not met within {timeout:?}");
    }

    struct FixedPlatform {
        supported: bool,
    }

    impl CaptureSessionPlatform for FixedPlatform {
        fn is_capture_supported(&self) -> bool {
            self.supported
        }
    }

    struct FixedClock {
        now_ms: u64,
    }

    impl CaptureSessionClock for FixedClock {
        fn now_ms(&self) -> u64 {
            self.now_ms
        }
    }

    struct RecordingEvents {
        states: Mutex<Vec<CaptureSessionSnapshot>>,
    }

    impl RecordingEvents {
        fn new() -> Self {
            Self {
                states: Mutex::new(Vec::new()),
            }
        }

        fn take_states(&self) -> Vec<CaptureSessionSnapshot> {
            self.states.lock().expect("lock").clone()
        }
    }

    impl CaptureSessionEvents for RecordingEvents {
        fn emit_state_changed(&self, state: &CaptureSessionSnapshot) {
            self.states.lock().expect("lock").push(state.clone());
        }
    }

    struct RecordingProcessing {
        started: AtomicUsize,
        stopping: AtomicUsize,
    }

    impl RecordingProcessing {
        fn new() -> Self {
            Self {
                started: AtomicUsize::new(0),
                stopping: AtomicUsize::new(0),
            }
        }
    }

    impl CaptureSessionProcessingHook for RecordingProcessing {
        fn on_capture_started(&self) {
            self.started.fetch_add(1, Ordering::SeqCst);
        }

        fn on_capture_stopping(&self) {
            self.stopping.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct MockDeviceSelection {
        selection: DeviceSelection,
    }

    impl DeviceSelectionService for MockDeviceSelection {
        fn list_devices(
            &self,
        ) -> Result<
            gijirec_domain::audio::AudioDeviceList,
            crate::device_selection::DeviceSelectionError,
        > {
            Err(crate::device_selection::DeviceSelectionError::internal(
                "mock",
            ))
        }

        fn get_selection(&self) -> DeviceSelection {
            self.selection.clone()
        }

        fn set_selection(
            &self,
            _selection: DeviceSelection,
        ) -> Result<DeviceSelection, crate::device_selection::DeviceSelectionError> {
            Err(crate::device_selection::DeviceSelectionError::internal(
                "mock",
            ))
        }

        fn set_ui_visible(&self, _visible: bool) {}
    }

    struct MockOrchestrator {
        phase: CapturePhase,
        start_ok: bool,
        start_gate: Option<Arc<(Mutex<bool>, std::sync::Condvar)>>,
        start_calls: AtomicUsize,
    }

    fn block_on_start_gate(gate: &Arc<(Mutex<bool>, std::sync::Condvar)>) {
        let mut started = gate.0.lock().expect("lock");
        *started = true;
        gate.1.notify_all();
        while *started {
            started = gate.1.wait(started).expect("wait");
        }
    }

    impl MockOrchestrator {
        fn idle_succeeds() -> Self {
            Self {
                phase: CapturePhase::Idle,
                start_ok: true,
                start_gate: None,
                start_calls: AtomicUsize::new(0),
            }
        }

        fn idle_fails() -> Self {
            Self {
                phase: CapturePhase::Idle,
                start_ok: false,
                start_gate: None,
                start_calls: AtomicUsize::new(0),
            }
        }

        fn with_blocking_start() -> Self {
            Self {
                phase: CapturePhase::Idle,
                start_ok: true,
                start_gate: Some(Arc::new((Mutex::new(false), std::sync::Condvar::new()))),
                start_calls: AtomicUsize::new(0),
            }
        }
    }

    impl CaptureOrchestrator for MockOrchestrator {
        fn start(&mut self) -> Result<(), CaptureError> {
            self.start_with_selection(&DeviceSelection::default())
        }

        fn start_with_selection(
            &mut self,
            _selection: &DeviceSelection,
        ) -> Result<(), CaptureError> {
            self.start_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = &self.start_gate {
                block_on_start_gate(gate);
            }
            if !self.start_ok {
                self.phase = CapturePhase::Error;
                return Err(CaptureError::Internal {
                    detail: "mock start failed".to_string(),
                });
            }
            self.phase = CapturePhase::Capturing;
            Ok(())
        }

        fn restart_with_selection(
            &mut self,
            selection: &DeviceSelection,
        ) -> Result<(), CaptureError> {
            self.start_with_selection(selection)
        }

        /* jscpd:ignore-start — mock CaptureOrchestrator tail matches lifecycle_hook tests */
        fn stop(&mut self) -> Result<(), CaptureError> {
            self.phase = CapturePhase::Idle;
            Ok(())
        }

        fn phase(&self) -> CapturePhase {
            self.phase
        }

        fn on_device_disconnected(&mut self) -> Result<(), CaptureError> {
            Ok(())
        }
        /* jscpd:ignore-end */
    }

    fn test_service(
        orchestrator: MockOrchestrator,
    ) -> (
        Arc<CaptureSessionService>,
        Arc<RecordingEvents>,
        Arc<RecordingProcessing>,
    ) {
        test_service_with_observability(orchestrator, Arc::new(NoopCaptureSessionObservability))
    }

    fn test_service_with_observability(
        orchestrator: MockOrchestrator,
        observability: Arc<dyn CaptureSessionObservability>,
    ) -> (
        Arc<CaptureSessionService>,
        Arc<RecordingEvents>,
        Arc<RecordingProcessing>,
    ) {
        test_service_with_observability_and_platform(orchestrator, observability, true)
    }

    fn test_service_with_observability_and_platform(
        orchestrator: MockOrchestrator,
        observability: Arc<dyn CaptureSessionObservability>,
        platform_supported: bool,
    ) -> (
        Arc<CaptureSessionService>,
        Arc<RecordingEvents>,
        Arc<RecordingProcessing>,
    ) {
        let orch = Arc::new(Mutex::new(orchestrator));
        let events = Arc::new(RecordingEvents::new());
        let processing = Arc::new(RecordingProcessing::new());
        let service = Arc::new(CaptureSessionService::new(
            orch,
            Arc::new(MockDeviceSelection {
                selection: DeviceSelection::default(),
            }),
            Arc::new(FixedPlatform {
                supported: platform_supported,
            }),
            processing.clone(),
            events.clone(),
            Arc::new(FixedClock { now_ms: 1 }),
            observability,
        ));
        (service, events, processing)
    }

    #[test]
    fn initial_state_is_idle_with_idle_capture() {
        let (service, _, _) = test_service(MockOrchestrator::idle_succeeds());
        let state = service.get_state();
        assert_eq!(state.session_phase, CaptureSessionPhase::Idle);
        assert!(!state.transition_busy);
        assert_eq!(state.capture_phase, CapturePhase::Idle);
    }

    #[test]
    fn double_start_while_transition_busy_returns_transition_busy() {
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let mut orch = MockOrchestrator::with_blocking_start();
        orch.start_gate = Some(Arc::clone(&gate));
        let (service, _, _) = test_service(orch);

        let gate_for_thread = Arc::clone(&gate);
        let service_for_thread = Arc::clone(&service);
        let handle = thread::spawn(move || {
            let _ = service_for_thread.start();
        });

        wait_until(Duration::from_secs(2), || {
            *gate_for_thread.0.lock().expect("lock")
        });

        let err = service
            .start()
            .expect_err("second start during transition must fail");
        assert_eq!(err.code, CaptureSessionErrorCode::TransitionBusy);

        {
            let mut flag = gate.0.lock().expect("lock");
            *flag = false;
            gate.1.notify_all();
        }
        handle.join().expect("join");
        assert_eq!(
            service.get_state().session_phase,
            CaptureSessionPhase::Active
        );
    }

    #[test]
    fn start_failure_keeps_session_idle() {
        let (service, events, _) = test_service(MockOrchestrator::idle_fails());
        let err = service.start().expect_err("start should fail");
        assert_eq!(err.code, CaptureSessionErrorCode::CaptureStartFailed);
        let state = service.get_state();
        assert_eq!(state.session_phase, CaptureSessionPhase::Idle);
        assert_eq!(state.capture_phase, CapturePhase::Idle);
        let emitted = events.take_states();
        assert!(
            emitted
                .iter()
                .any(|s| s.session_phase == CaptureSessionPhase::Starting),
            "expected starting emission before failure"
        );
        assert!(
            emitted
                .last()
                .is_some_and(|s| s.session_phase == CaptureSessionPhase::Idle),
            "expected final idle emission"
        );
    }

    #[test]
    fn start_when_already_active_is_idempotent() {
        let (service, events, _) = test_service(MockOrchestrator::idle_succeeds());
        service.start().expect("first start");
        let emission_count_after_first = events.take_states().len();
        assert_eq!(
            emission_count_after_first, 2,
            "expected starting then active emissions"
        );
        let state = service.start().expect("second start");
        assert_eq!(state.session_phase, CaptureSessionPhase::Active);
        assert_eq!(
            events.take_states().len(),
            emission_count_after_first,
            "idempotent start must not emit state-changed again"
        );
    }

    #[test]
    fn successful_start_emits_state_changed_on_each_transition() {
        let (service, events, _) = test_service(MockOrchestrator::idle_succeeds());
        service.start().expect("start");
        let phases: Vec<CaptureSessionPhase> = events
            .take_states()
            .into_iter()
            .map(|s| s.session_phase)
            .collect();
        assert_eq!(
            phases,
            vec![CaptureSessionPhase::Starting, CaptureSessionPhase::Active],
            "state-changed must fire on starting and active (req 2.4 / D-CaptureSessionService)"
        );
    }

    #[test]
    fn start_emits_observability_phase_transitions() {
        let obs = Arc::new(RecordingCaptureSessionObservability::new());
        let (service, _, _) =
            test_service_with_observability(MockOrchestrator::idle_succeeds(), obs.clone());

        service.start().expect("start");

        let transitions = obs.phase_transitions.lock().expect("lock").clone();
        assert!(
            transitions.contains(&(CaptureSessionPhase::Idle, CaptureSessionPhase::Starting)),
            "expected idle→starting, got {transitions:?}"
        );
        assert!(
            transitions.contains(&(CaptureSessionPhase::Starting, CaptureSessionPhase::Active)),
        );
        let busy = obs.transition_busy_flags.lock().expect("lock").clone();
        assert_eq!(busy, vec![true, false], "starting is busy, active is not");
    }

    #[test]
    fn start_invokes_capture_started_hook_only_not_stopping() {
        let (service, _, processing) = test_service(MockOrchestrator::idle_succeeds());
        service.start().expect("start");
        assert_eq!(processing.started.load(Ordering::SeqCst), 1);
        assert_eq!(processing.stopping.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unsupported_platform_rejects_start() {
        let (service, _, _) = test_service_with_observability_and_platform(
            MockOrchestrator::idle_succeeds(),
            Arc::new(NoopCaptureSessionObservability),
            false,
        );
        let err = service.start().expect_err("linux must reject start");
        assert_eq!(err.code, CaptureSessionErrorCode::UnsupportedPlatform);
        assert_eq!(service.get_state().session_phase, CaptureSessionPhase::Idle);
    }
}
