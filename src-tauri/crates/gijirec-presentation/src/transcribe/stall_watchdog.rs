//! Stall detection while capture is active and the transcribe worker stops making progress.
//!
//! Fires `INFERENCE_FAILED` only when the whisper engine fails to load within
//! [`ENGINE_LOAD_TIMEOUT`] or an in-flight inference exceeds [`INFERENCE_TIMEOUT`].
//! Ambient PCM above the VAD threshold does not affect stall detection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gijirec_application::transcribe::TranscribeOrchestrator;
use gijirec_domain::transcribe::{TranscribeError, TranscribePhase};

use super::event_emitter::TranscribeEventEmitter;
use super::observability;

/// VAD near-silence RMS threshold (matches `transcribe_worker::SILENCE_RMS_THRESHOLD`).
pub const SILENCE_RMS_THRESHOLD: f32 = 0.008;

/// Fixed batch inference interval (matches `transcribe_worker::BATCH_INTERVAL` in production).
pub const BATCH_INTERVAL: Duration = Duration::from_secs(30);

/// Latency budget retained for docs/tests (30 s batch window + 5 s margin + 3 s poll slack).
/// Stall firing relies on [`ENGINE_LOAD_TIMEOUT`] and [`INFERENCE_TIMEOUT`] only; idle gaps
/// between batch cycles must stay below this budget so tests document the 30 s window.
pub const STALL_THRESHOLD: Duration = Duration::from_secs(38);

/// Whisper context load happens on the worker after `transcribing` begins.
/// Do not treat that interval as a no-block stall.
pub const ENGINE_LOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Do not treat in-flight whisper.cpp work as a no-block stall.
/// whisper.cpp often emits only 0% at start and 100% at end, so CPU encode of
/// kotoba-whisper can run for minutes without further progress callbacks.
pub const INFERENCE_TIMEOUT: Duration = Duration::from_secs(600);

/// Poll interval while the stall watchdog is armed.
pub const STALL_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Injectable clock returning capture-relative milliseconds.
pub type StallClock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Port for orchestrator failure on stall (mockable in unit tests).
pub trait TranscribeStallOrchestrator: Send {
    /// Returns true when capture is active and transcription is in progress.
    fn is_watchable(&self) -> bool;
    /// Transitions orchestrator to `error` after a stall is detected.
    fn fail_inference_stall(&mut self);
}

/// Bridges a shared emitter cell into [`TranscribeEventEmitter`].
pub struct SharedTranscribeEmitter(Arc<Mutex<Arc<dyn TranscribeEventEmitter>>>);

impl SharedTranscribeEmitter {
    pub fn new(emitter: Arc<Mutex<Arc<dyn TranscribeEventEmitter>>>) -> Self {
        Self(emitter)
    }
}

impl TranscribeEventEmitter for SharedTranscribeEmitter {
    fn emit_phase_changed(
        &self,
        phase: TranscribePhase,
    ) -> Result<(), super::event_emitter::TranscribeEmitError> {
        self.0
            .lock()
            .expect("lock emitter")
            .emit_phase_changed(phase)
    }

    fn emit_model_progress(
        &self,
        progress: &gijirec_application::transcribe::ModelDownloadProgress,
    ) -> Result<(), super::event_emitter::TranscribeEmitError> {
        self.0
            .lock()
            .expect("lock emitter")
            .emit_model_progress(progress)
    }

    fn emit_error(
        &self,
        error: &TranscribeError,
    ) -> Result<(), super::event_emitter::TranscribeEmitError> {
        self.0.lock().expect("lock emitter").emit_error(error)
    }
}

/// [`TranscribeStallOrchestrator`] adapter over [`TranscribeOrchestrator`].
pub struct OrchestratorStallAdapter {
    orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    upstream_capturing: AtomicBool,
}

impl OrchestratorStallAdapter {
    pub fn new(orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>) -> Self {
        Self {
            orchestrator,
            upstream_capturing: AtomicBool::new(false),
        }
    }

    pub fn set_upstream_capturing(&self, capturing: bool) {
        self.upstream_capturing.store(capturing, Ordering::SeqCst);
    }
}

impl TranscribeStallOrchestrator for OrchestratorStallAdapter {
    fn is_watchable(&self) -> bool {
        self.upstream_capturing.load(Ordering::SeqCst)
            && self.orchestrator.lock().expect("lock orchestrator").phase()
                == TranscribePhase::Transcribing
    }

    fn fail_inference_stall(&mut self) {
        self.upstream_capturing.store(false, Ordering::SeqCst);
        self.orchestrator
            .lock()
            .expect("lock orchestrator")
            .fail_inference();
    }
}

/// Background poll loop for [`TranscribeStallWatchdog`].
pub struct StallWatchdogRuntime {
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl StallWatchdogRuntime {
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(true)),
            handle: Mutex::new(None),
        }
    }

    pub fn start<O, E>(&self, watchdog: Arc<TranscribeStallWatchdog<O, E>>)
    where
        O: TranscribeStallOrchestrator + Send + 'static,
        E: TranscribeEventEmitter + Send + Sync + 'static,
    {
        let Ok(mut guard) = self.handle.lock() else {
            return;
        };
        if let Some(handle) = guard.take() {
            self.stop.store(true, Ordering::SeqCst);
            let _ = handle.join();
        }
        self.stop.store(false, Ordering::SeqCst);
        let stop = Arc::clone(&self.stop);
        *guard = Some(thread::spawn(move || poll_until_stopped(watchdog, stop)));
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.handle.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

fn poll_until_stopped<O, E>(watchdog: Arc<TranscribeStallWatchdog<O, E>>, stop: Arc<AtomicBool>)
where
    O: TranscribeStallOrchestrator + Send + 'static,
    E: TranscribeEventEmitter + Send + Sync + 'static,
{
    while !stop.load(Ordering::SeqCst) {
        watchdog.poll();
        thread::sleep(STALL_POLL_INTERVAL);
    }
}

impl Default for StallWatchdogRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Detects transcription stalls and surfaces `INFERENCE_FAILED` to the UI.
pub struct TranscribeStallWatchdog<O, E> {
    orchestrator: Arc<Mutex<O>>,
    emitter: Arc<E>,
    clock: StallClock,
    last_progress_ms: Mutex<u64>,
    armed_at_ms: Mutex<u64>,
    armed: AtomicBool,
    fired: AtomicBool,
    engine_ready: AtomicBool,
    inference_in_flight: AtomicBool,
}

impl<O: TranscribeStallOrchestrator, E: TranscribeEventEmitter> TranscribeStallWatchdog<O, E> {
    pub fn new(orchestrator: Arc<Mutex<O>>, emitter: Arc<E>, clock: StallClock) -> Self {
        let now = clock();
        Self {
            orchestrator,
            emitter,
            clock,
            last_progress_ms: Mutex::new(now),
            armed_at_ms: Mutex::new(now),
            armed: AtomicBool::new(false),
            fired: AtomicBool::new(false),
            engine_ready: AtomicBool::new(false),
            inference_in_flight: AtomicBool::new(false),
        }
    }

    /// Arms the watchdog for a new capture/transcribe session.
    pub fn arm(&self) {
        let now = (self.clock)();
        *self.last_progress_ms.lock().expect("lock progress") = now;
        *self.armed_at_ms.lock().expect("lock armed_at") = now;
        self.engine_ready.store(false, Ordering::SeqCst);
        self.inference_in_flight.store(false, Ordering::SeqCst);
        self.fired.store(false, Ordering::SeqCst);
        self.armed.store(true, Ordering::SeqCst);
    }

    /// Disarms the watchdog (capture stop or phase leave).
    pub fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
    }

    /// Resets the progress clock when a transcript block is appended.
    pub fn on_block_appended(&self) {
        if !self.armed.load(Ordering::SeqCst) {
            return;
        }
        self.inference_in_flight.store(false, Ordering::SeqCst);
        *self.last_progress_ms.lock().expect("lock progress") = (self.clock)();
    }

    /// Marks the whisper engine loaded and restarts the no-block window.
    pub fn on_engine_ready(&self) {
        if !self.armed.load(Ordering::SeqCst) || self.fired.load(Ordering::SeqCst) {
            return;
        }
        self.engine_ready.store(true, Ordering::SeqCst);
        *self.last_progress_ms.lock().expect("lock progress") = (self.clock)();
        observability::log_engine_ready();
    }

    /// Resets progress when worker inference completes (including empty windows).
    pub fn on_inference_success(&self) {
        if !self.armed.load(Ordering::SeqCst) {
            return;
        }
        self.inference_in_flight.store(false, Ordering::SeqCst);
        *self.last_progress_ms.lock().expect("lock progress") = (self.clock)();
    }

    /// Marks whisper.cpp as busy so CPU inference is not treated as a no-block stall.
    pub fn on_inference_attempted(&self) {
        if !self.armed.load(Ordering::SeqCst) {
            return;
        }
        self.inference_in_flight.store(true, Ordering::SeqCst);
        *self.last_progress_ms.lock().expect("lock progress") = (self.clock)();
        observability::log_inference_started();
    }

    /// Extends the in-flight window when whisper.cpp reports encoder/decoder progress.
    pub fn on_inference_progress(&self) {
        if !self.armed.load(Ordering::SeqCst) || !self.inference_in_flight.load(Ordering::SeqCst) {
            return;
        }
        *self.last_progress_ms.lock().expect("lock progress") = (self.clock)();
    }

    /// Polls stall conditions and fires once when the threshold is exceeded.
    pub fn poll(&self) -> bool {
        if !self.armed.load(Ordering::SeqCst) || self.fired.load(Ordering::SeqCst) {
            return false;
        }

        let orch = self.orchestrator.lock().expect("lock orchestrator");
        if !orch.is_watchable() {
            return false;
        }
        drop(orch);

        let now = (self.clock)();
        if !self.engine_ready.load(Ordering::SeqCst) {
            let armed_at = *self.armed_at_ms.lock().expect("lock armed_at");
            if now.saturating_sub(armed_at) >= ENGINE_LOAD_TIMEOUT.as_millis() as u64 {
                self.fire();
                return true;
            }
            return false;
        }

        let last_progress = *self.last_progress_ms.lock().expect("lock progress");
        let elapsed_ms = now.saturating_sub(last_progress);

        if self.inference_in_flight.load(Ordering::SeqCst) {
            if elapsed_ms >= INFERENCE_TIMEOUT.as_millis() as u64 {
                self.fire();
                return true;
            }
            return false;
        }

        false
    }

    pub fn has_fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }

    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }

    fn fire(&self) {
        if self.fired.swap(true, Ordering::SeqCst) {
            return;
        }

        let error = TranscribeError::InferenceFailed {
            detail: "transcription stalled: no blocks within latency window".to_string(),
        };

        observability::log_stall_detected();
        observability::log_transcribe_error(&error);
        let _ = self.emitter.emit_error(&error);
        observability::log_phase_transition(TranscribePhase::Error);
        let _ = self.emitter.emit_phase_changed(TranscribePhase::Error);

        let mut orch = self.orchestrator.lock().expect("lock orchestrator");
        orch.fail_inference_stall();
    }
}

/// Computes RMS for a normalized PCM chunk (`[-1.0, 1.0]`).
pub fn chunk_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|sample| sample * sample).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::observability::with_isolated_transcribe_observability;
    use gijirec_application::transcribe::ModelDownloadProgress;
    use gijirec_domain::transcribe::UserFacingTranscribeError;
    use std::sync::atomic::AtomicU64;

    struct MockOrchestrator {
        watchable: bool,
        phase: TranscribePhase,
        fail_count: u32,
    }

    impl MockOrchestrator {
        fn watchable() -> Self {
            Self {
                watchable: true,
                phase: TranscribePhase::Transcribing,
                fail_count: 0,
            }
        }
    }

    impl TranscribeStallOrchestrator for MockOrchestrator {
        fn is_watchable(&self) -> bool {
            self.watchable
        }

        fn fail_inference_stall(&mut self) {
            self.fail_count += 1;
            self.phase = TranscribePhase::Error;
        }
    }

    #[derive(Default)]
    struct MockEmitter {
        phases: Mutex<Vec<TranscribePhase>>,
        errors: Mutex<Vec<UserFacingTranscribeError>>,
    }

    impl TranscribeEventEmitter for MockEmitter {
        fn emit_phase_changed(
            &self,
            phase: TranscribePhase,
        ) -> Result<(), super::super::event_emitter::TranscribeEmitError> {
            self.phases.lock().expect("lock phases").push(phase);
            Ok(())
        }

        fn emit_model_progress(
            &self,
            _progress: &ModelDownloadProgress,
        ) -> Result<(), super::super::event_emitter::TranscribeEmitError> {
            Ok(())
        }

        fn emit_error(
            &self,
            error: &TranscribeError,
        ) -> Result<(), super::super::event_emitter::TranscribeEmitError> {
            self.errors
                .lock()
                .expect("lock errors")
                .push(error.to_user_facing());
            Ok(())
        }
    }

    fn test_clock() -> (StallClock, Arc<AtomicU64>) {
        let time = Arc::new(AtomicU64::new(0));
        let clock: StallClock = {
            let time = Arc::clone(&time);
            Arc::new(move || time.load(Ordering::SeqCst))
        };
        (clock, time)
    }

    #[test]
    fn stall_threshold_covers_batch_interval() {
        assert!(
            STALL_THRESHOLD >= BATCH_INTERVAL,
            "stall docs/tests must tolerate full 30 s batch idle gaps"
        );
        assert_eq!(STALL_THRESHOLD, Duration::from_secs(38));
    }

    #[test]
    fn does_not_fire_during_batch_interval_idle_without_inference() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            watchdog.on_engine_ready();
            watchdog.on_inference_success();

            time.store(BATCH_INTERVAL.as_millis() as u64 + 5_000, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "30 s batch idle between cycles must not surface a stall"
            );
            assert!(emitter.errors.lock().expect("lock errors").is_empty());
        });
    }

    #[test]
    fn does_not_fire_during_noise_without_inference() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            watchdog.on_engine_ready();
            watchdog.on_block_appended();

            time.store(25_100, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "ambient PCM without a worker inference attempt must not trigger stall detection"
            );

            assert!(emitter.errors.lock().expect("lock errors").is_empty());
            assert!(emitter.phases.lock().expect("lock phases").is_empty());
            assert_eq!(orch.lock().expect("lock orchestrator").fail_count, 0);
            assert!(!watchdog.has_fired());
        });
    }

    #[test]
    fn engine_ready_resets_stall_timer_without_pending_inference() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            time.store(7_000, Ordering::SeqCst);
            watchdog.on_engine_ready();
            time.store(14_500, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "engine ready must reset the no-block window so model load is not a stall"
            );

            time.store(25_100, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "idle capture after engine ready must not surface a stall without inference"
            );
        });
    }

    #[test]
    fn does_not_fire_block_stall_before_engine_ready() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            time.store(8_100, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "whisper context load must not count as a no-block stall"
            );
            assert!(emitter.errors.lock().expect("lock errors").is_empty());
        });
    }

    #[test]
    fn fires_load_timeout_when_engine_never_ready() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            time.store(
                ENGINE_LOAD_TIMEOUT.as_millis() as u64 + 100,
                Ordering::SeqCst,
            );
            assert!(
                watchdog.poll(),
                "engine load that never completes must surface a stall"
            );
            assert_eq!(emitter.errors.lock().expect("lock errors").len(), 1);
        });
    }

    #[test]
    fn resets_progress_on_block_append_and_inference_success() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            watchdog.on_engine_ready();
            watchdog.on_inference_attempted();

            time.store(7_000, Ordering::SeqCst);
            assert!(!watchdog.poll());

            watchdog.on_block_appended();
            time.store(24_999, Ordering::SeqCst);
            assert!(!watchdog.poll(), "recent block append resets stall timer");

            watchdog.on_inference_success();
            time.store(42_998, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "recent inference success resets stall timer"
            );
        });
    }

    #[test]
    fn does_not_fire_while_inference_in_flight_until_timeout() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            watchdog.on_engine_ready();
            watchdog.on_inference_attempted();

            time.store(8_100, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "in-flight CPU inference must not be treated as a no-block stall"
            );
            assert!(emitter.errors.lock().expect("lock errors").is_empty());

            time.store(INFERENCE_TIMEOUT.as_millis() as u64 + 100, Ordering::SeqCst);
            assert!(
                watchdog.poll(),
                "inference that never returns must surface a stall"
            );
            assert_eq!(emitter.errors.lock().expect("lock errors").len(), 1);
        });
    }

    #[test]
    fn inference_progress_extends_in_flight_timeout() {
        with_isolated_transcribe_observability(|| {
            let (clock, time) = test_clock();
            let orch = Arc::new(Mutex::new(MockOrchestrator::watchable()));
            let emitter = Arc::new(MockEmitter::default());
            let watchdog = TranscribeStallWatchdog::new(orch.clone(), emitter.clone(), clock);

            watchdog.arm();
            watchdog.on_engine_ready();
            watchdog.on_inference_attempted();
            time.store(80_000, Ordering::SeqCst);
            watchdog.on_inference_progress();
            time.store(160_000, Ordering::SeqCst);
            assert!(
                !watchdog.poll(),
                "slow CPU inference that still reports progress is not a stall"
            );

            time.store(
                160_000 + INFERENCE_TIMEOUT.as_millis() as u64 + 100,
                Ordering::SeqCst,
            );
            assert!(
                watchdog.poll(),
                "inference that stops reporting progress must stall"
            );
        });
    }

    #[test]
    fn chunk_rms_matches_worker_formula() {
        let audible = vec![0.1, -0.1, 0.2, -0.2];
        let silence = vec![0.0, 0.0, 0.0];
        assert!(chunk_rms(&audible) > SILENCE_RMS_THRESHOLD);
        assert!(chunk_rms(&silence) < SILENCE_RMS_THRESHOLD);
    }
}
