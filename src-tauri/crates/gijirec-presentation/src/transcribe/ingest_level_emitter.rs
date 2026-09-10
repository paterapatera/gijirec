//! 1 Hz ingest-level dBFS aggregation and Tauri event emission.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use super::pcm_ingest_consumer::PcmChunkRmsCallback;

/// Tauri event name for ingest-level meter updates.
pub const INGEST_LEVEL_EVENT: &str = "capture-audio-controls://ingest-level";

/// dBFS floor when RMS is zero or negative.
pub const DBFS_FLOOR: f32 = -120.0;

/// RMS aggregation window before each emit.
pub const AGGREGATION_WINDOW: Duration = Duration::from_secs(1);

/// Nominal emit interval while capturing and supplying ingest.
pub const MIN_EMIT_INTERVAL: Duration = Duration::from_secs(1);

/// Maximum emit interval under resource pressure (meter degrade only).
pub const MAX_EMIT_INTERVAL: Duration = Duration::from_secs(2);

/// Payload for `capture-audio-controls://ingest-level` per contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IngestLevelChangedPayload {
    pub level_dbfs: f32,
    pub timestamp_ms: u64,
}

/// Clock source for capture/session-relative timestamp in events.
pub type TimestampClock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Abstract emitter interface for ingest-level events (mockable in tests).
pub trait IngestLevelEventEmitter: Send + Sync {
    fn emit_ingest_level(&self, payload: IngestLevelChangedPayload) -> Result<(), String>;
}

/// Production implementation backed by [`AppHandle`].
pub struct TauriIngestLevelEventEmitter<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriIngestLevelEventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> IngestLevelEventEmitter for TauriIngestLevelEventEmitter<R> {
    fn emit_ingest_level(&self, payload: IngestLevelChangedPayload) -> Result<(), String> {
        self.app
            .emit(INGEST_LEVEL_EVENT, payload)
            .map_err(|e| e.to_string())
    }
}

/// Aggregates post-gain RMS from [`PcmIngestConsumer`] and emits dBFS metadata at 1 Hz.
pub struct IngestLevelEmitter {
    inner: Arc<Mutex<IngestLevelEmitterState>>,
}

struct IngestLevelEmitterState {
    emitter: Arc<dyn IngestLevelEventEmitter>,
    clock: Option<TimestampClock>,
    sum_sq: f64,
    chunk_count: u32,
    supplying: bool,
    under_pressure: bool,
    window_anchor: Option<Instant>,
    last_emit: Option<Instant>,
    skipped_total: u64,
}

impl IngestLevelEmitterState {
    fn emit_interval(&self) -> Duration {
        if self.under_pressure {
            MAX_EMIT_INTERVAL
        } else {
            MIN_EMIT_INTERVAL
        }
    }

    fn record_pcm_rms(&mut self, rms: f32) {
        if !rms.is_finite() || rms < 0.0 {
            return;
        }
        self.sum_sq += (rms as f64) * (rms as f64);
        self.chunk_count += 1;
    }

    fn window_rms(&self) -> f32 {
        if self.chunk_count == 0 {
            return 0.0;
        }
        (self.sum_sq / self.chunk_count as f64).sqrt() as f32
    }

    fn reset_window(&mut self) {
        self.sum_sq = 0.0;
        self.chunk_count = 0;
    }

    fn reset_emit_schedule(&mut self) {
        self.reset_window();
        self.window_anchor = None;
        self.last_emit = None;
    }

    fn try_emit(&mut self, now: Instant, count_skip: bool) {
        if !self.supplying {
            if count_skip {
                self.skipped_total += 1;
            }
            self.reset_emit_schedule();
            return;
        }

        if self.window_anchor.is_none() {
            self.window_anchor = Some(now);
            return;
        }

        let anchor = self
            .last_emit
            .unwrap_or_else(|| self.window_anchor.unwrap());
        if now.duration_since(anchor) < self.emit_interval() {
            return;
        }

        let level_dbfs = rms_to_dbfs(self.window_rms());
        let timestamp_ms = self.clock.as_ref().map(|c| c()).unwrap_or(0);
        let payload = IngestLevelChangedPayload {
            level_dbfs,
            timestamp_ms,
        };

        if self.emitter.emit_ingest_level(payload).is_ok() {
            self.last_emit = Some(now);
            self.window_anchor = Some(now);
            self.reset_window();
        }
    }
}

impl IngestLevelEmitter {
    pub fn new(emitter: Arc<dyn IngestLevelEventEmitter>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(IngestLevelEmitterState {
                emitter,
                clock: None,
                sum_sq: 0.0,
                chunk_count: 0,
                supplying: false,
                under_pressure: false,
                window_anchor: None,
                last_emit: None,
                skipped_total: 0,
            })),
        }
    }

    pub fn with_clock(emitter: Arc<dyn IngestLevelEventEmitter>, clock: TimestampClock) -> Self {
        let emitter = Self::new(emitter);
        if let Ok(mut state) = emitter.inner.lock() {
            state.clock = Some(clock);
        }
        emitter
    }

    /// Whether ingest is being supplied (`capturing` and an audio source is available).
    pub fn set_supplying(&self, supplying: bool) {
        if let Ok(mut state) = self.inner.lock() {
            state.supplying = supplying;
            if !supplying {
                state.reset_emit_schedule();
            }
        }
    }

    /// Slows meter updates to 2 s under resource pressure without affecting ingest.
    pub fn set_under_pressure(&self, under_pressure: bool) {
        if let Ok(mut state) = self.inner.lock() {
            state.under_pressure = under_pressure;
        }
    }

    /// Total emits skipped because ingest was not being supplied.
    pub fn ingest_level_emit_skipped_total(&self) -> u64 {
        self.inner
            .lock()
            .map(|state| state.skipped_total)
            .unwrap_or(0)
    }

    /// Records post-gain chunk RMS for the current aggregation window.
    pub fn record_pcm_rms(&self, rms: f32) {
        let now = Instant::now();
        self.record_pcm_rms_at(rms, now);
    }

    /// Records RMS and attempts emit using an explicit clock (tests).
    pub fn record_pcm_rms_at(&self, rms: f32, now: Instant) {
        if let Ok(mut state) = self.inner.lock() {
            state.record_pcm_rms(rms);
            state.try_emit(now, false);
        }
    }

    /// Attempts emit when the aggregation interval has elapsed (timer tick).
    pub fn tick_at(&self, now: Instant) {
        if let Ok(mut state) = self.inner.lock() {
            state.try_emit(now, true);
        }
    }

    /// Callback for wiring into [`PcmIngestConsumer::set_pcm_rms_callback`].
    pub fn pcm_rms_callback(&self) -> PcmChunkRmsCallback {
        let inner = Arc::clone(&self.inner);
        Arc::new(move |rms| {
            let now = Instant::now();
            if let Ok(mut state) = inner.lock() {
                state.record_pcm_rms(rms);
                state.try_emit(now, false);
            }
        })
    }
}

/// Converts linear RMS to dBFS (`20 * log10(rms)`), flooring at −120.
pub fn rms_to_dbfs(rms: f32) -> f32 {
    if rms <= 0.0 {
        DBFS_FLOOR
    } else {
        20.0 * rms.log10()
    }
}

/// Aggregates equal-weight chunk RMS values over a window.
pub fn aggregate_window_rms(chunk_rms_values: &[f32]) -> f32 {
    if chunk_rms_values.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = chunk_rms_values
        .iter()
        .filter(|rms| rms.is_finite() && **rms >= 0.0)
        .map(|rms| (*rms as f64) * (*rms as f64))
        .sum();
    let count = chunk_rms_values
        .iter()
        .filter(|rms| rms.is_finite() && **rms >= 0.0)
        .count();
    if count == 0 {
        return 0.0;
    }
    (sum_sq / count as f64).sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingIngestLevelEmitter {
        events: Mutex<Vec<IngestLevelChangedPayload>>,
    }

    impl RecordingIngestLevelEmitter {
        fn events(&self) -> Vec<IngestLevelChangedPayload> {
            self.events.lock().unwrap().clone()
        }
    }

    impl IngestLevelEventEmitter for RecordingIngestLevelEmitter {
        fn emit_ingest_level(&self, payload: IngestLevelChangedPayload) -> Result<(), String> {
            self.events.lock().unwrap().push(payload);
            Ok(())
        }
    }

    fn test_emitter() -> (IngestLevelEmitter, Arc<RecordingIngestLevelEmitter>) {
        let recorder = Arc::new(RecordingIngestLevelEmitter::default());
        let emitter = IngestLevelEmitter::with_clock(
            Arc::clone(&recorder) as Arc<dyn IngestLevelEventEmitter>,
            Arc::new(|| 1_500),
        );
        (emitter, recorder)
    }

    #[test]
    fn rms_to_dbfs_floors_at_minus_120_for_non_positive() {
        assert_eq!(rms_to_dbfs(0.0), DBFS_FLOOR);
        assert_eq!(rms_to_dbfs(-0.5), DBFS_FLOOR);
    }

    #[test]
    fn rms_to_dbfs_converts_linear_rms() {
        assert!((rms_to_dbfs(0.1) - (-20.0)).abs() < 1e-4);
        assert!((rms_to_dbfs(1.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn aggregates_one_second_window_before_emit() {
        let (emitter, recorder) = test_emitter();
        emitter.set_supplying(true);

        let t0 = Instant::now();
        emitter.record_pcm_rms_at(0.1, t0);
        emitter.tick_at(t0 + Duration::from_millis(500));
        assert!(
            recorder.events().is_empty(),
            "must not emit before 1 s window"
        );

        emitter.record_pcm_rms_at(0.2, t0 + Duration::from_millis(900));
        emitter.tick_at(t0 + Duration::from_secs(1));
        let events = recorder.events();
        assert_eq!(events.len(), 1);

        let expected_rms = aggregate_window_rms(&[0.1, 0.2]);
        let expected_dbfs = rms_to_dbfs(expected_rms);
        assert!((events[0].level_dbfs - expected_dbfs).abs() < 1e-3);
        assert_eq!(events[0].timestamp_ms, 1_500);
    }

    #[test]
    fn skips_emit_when_not_supplying() {
        let (emitter, recorder) = test_emitter();
        emitter.set_supplying(false);

        let t0 = Instant::now();
        emitter.record_pcm_rms_at(0.5, t0);
        emitter.tick_at(t0 + Duration::from_secs(1));
        emitter.tick_at(t0 + Duration::from_secs(2));

        assert!(recorder.events().is_empty());
        assert_eq!(emitter.ingest_level_emit_skipped_total(), 2);
    }

    #[test]
    fn emits_silence_floor_when_supplying_without_chunks() {
        let (emitter, recorder) = test_emitter();
        emitter.set_supplying(true);

        let t0 = Instant::now();
        emitter.tick_at(t0);
        emitter.tick_at(t0 + Duration::from_secs(1));

        let events = recorder.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level_dbfs, DBFS_FLOOR);
    }

    #[test]
    fn under_pressure_extends_emit_interval_to_two_seconds() {
        let (emitter, recorder) = test_emitter();
        emitter.set_supplying(true);
        emitter.set_under_pressure(true);

        let t0 = Instant::now();
        emitter.record_pcm_rms_at(0.1, t0);
        emitter.tick_at(t0 + Duration::from_secs(1));
        assert!(recorder.events().is_empty(), "pressure mode waits 2 s");

        emitter.tick_at(t0 + Duration::from_secs(2));
        assert_eq!(recorder.events().len(), 1);
    }
}
