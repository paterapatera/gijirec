use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Single-session unprocessed PCM design limit (1 h @ 16 kHz mono).
pub const MAX_PCM_RETENTION_SAMPLES: usize = 57_600_000;

/// Public alias kept for existing exports and documentation alignment.
pub const MAX_PCM_BUFFER_SAMPLES: usize = MAX_PCM_RETENTION_SAMPLES;

const PCM_SAMPLE_RATE_HZ: f64 = 16_000.0;

/// Seconds of mono PCM represented by `sample_count` at 16 kHz.
pub(crate) fn samples_to_backlog_seconds(sample_count: usize) -> f64 {
    sample_count as f64 / PCM_SAMPLE_RATE_HZ
}

pub(crate) struct PcmBufferState {
    pub(crate) samples: VecDeque<f32>,
    pub(crate) samples_before_buffer: u64,
    /// When set (tests), overrides [`MAX_PCM_RETENTION_SAMPLES`].
    pub(crate) retention_limit_samples: Option<usize>,
    pub(crate) retention_limit_reached: AtomicBool,
}

impl PcmBufferState {
    pub(crate) fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            samples_before_buffer: 0,
            retention_limit_samples: None,
            retention_limit_reached: AtomicBool::new(false),
        }
    }

    pub(crate) fn effective_retention_limit(&self) -> usize {
        self.retention_limit_samples
            .unwrap_or(MAX_PCM_RETENTION_SAMPLES)
    }

    pub(crate) fn at_retention_limit(&self) -> bool {
        self.samples.len() >= self.effective_retention_limit()
    }
}

pub(crate) fn pcm_buffer_sample_count(pcm_buffer: &Arc<Mutex<PcmBufferState>>) -> usize {
    pcm_buffer.lock().expect("pcm buffer lock").samples.len()
}

pub(crate) fn pcm_buffer_has_remaining(pcm_buffer: &Arc<Mutex<PcmBufferState>>) -> bool {
    !pcm_buffer
        .lock()
        .expect("pcm buffer lock")
        .samples
        .is_empty()
}

/// Optional PCM drain side-effects (retention limit signal, UI backlog estimate).
pub(crate) struct PcmDrainCallbacks {
    pub on_retention_limit: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_pcm_backlog_seconds: Option<Arc<dyn Fn(f64) + Send + Sync>>,
}

struct PcmBacklogReportState {
    last_reported_seconds: f64,
    last_report_at: std::time::Instant,
}

struct PcmBacklogReportOptions {
    min_interval: Duration,
    force: bool,
}

impl PcmBacklogReportState {
    fn new() -> Self {
        Self {
            last_reported_seconds: f64::NAN,
            last_report_at: std::time::Instant::now(),
        }
    }

    fn maybe_report(
        &mut self,
        pcm_buffer: &Arc<Mutex<PcmBufferState>>,
        callback: Option<&Arc<dyn Fn(f64) + Send + Sync>>,
        options: PcmBacklogReportOptions,
    ) {
        let Some(report) = callback else {
            return;
        };
        if !options.force && self.last_report_at.elapsed() < options.min_interval {
            return;
        }
        let sample_count = pcm_buffer.lock().expect("pcm buffer lock").samples.len();
        let seconds = samples_to_backlog_seconds(sample_count);
        let rounded = (seconds * 10.0).round() / 10.0;
        if !options.force && (rounded - self.last_reported_seconds).abs() < 0.05 {
            return;
        }
        self.last_reported_seconds = rounded;
        self.last_report_at = std::time::Instant::now();
        report(rounded);
    }
}

pub(crate) fn drain_pcm_loop(
    mut consumer: rtrb::Consumer<f32>,
    pcm_buffer: Arc<Mutex<PcmBufferState>>,
    running: Arc<AtomicBool>,
    callbacks: PcmDrainCallbacks,
) {
    let mut backlog_report = PcmBacklogReportState::new();
    const BACKLOG_REPORT_INTERVAL: Duration = Duration::from_secs(1);

    while running.load(Ordering::SeqCst) {
        let popped = {
            let mut state = pcm_buffer.lock().expect("pcm buffer lock");
            drain_consumer(&mut consumer, &mut state, true)
        };
        maybe_signal_retention_limit(&pcm_buffer, callbacks.on_retention_limit.as_ref());
        backlog_report.maybe_report(
            &pcm_buffer,
            callbacks.on_pcm_backlog_seconds.as_ref(),
            PcmBacklogReportOptions {
                min_interval: BACKLOG_REPORT_INTERVAL,
                force: false,
            },
        );
        if popped == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }

    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    drain_consumer(&mut consumer, &mut state, false);
    backlog_report.maybe_report(
        &pcm_buffer,
        callbacks.on_pcm_backlog_seconds.as_ref(),
        PcmBacklogReportOptions {
            min_interval: Duration::ZERO,
            force: true,
        },
    );
}

pub(crate) fn drain_front_samples(state: &mut PcmBufferState, cut: usize) -> (Vec<f32>, u64) {
    let base_samples = state.samples_before_buffer;
    let pcm: Vec<f32> = state.samples.drain(..cut).collect();
    state.samples_before_buffer += cut as u64;
    (pcm, base_samples)
}

fn maybe_signal_retention_limit(
    pcm_buffer: &Arc<Mutex<PcmBufferState>>,
    on_retention_limit: Option<&Arc<dyn Fn() + Send + Sync>>,
) {
    let Some(callback) = on_retention_limit else {
        return;
    };
    let state = pcm_buffer.lock().expect("pcm buffer lock");
    if state.at_retention_limit() && !state.retention_limit_reached.swap(true, Ordering::SeqCst) {
        callback();
    }
}

/// When `enforce_retention_limit` is true, stops deque growth at the design limit.
pub(crate) fn drain_consumer(
    consumer: &mut rtrb::Consumer<f32>,
    state: &mut PcmBufferState,
    enforce_retention_limit: bool,
) -> usize {
    let mut popped = 0usize;
    loop {
        if enforce_retention_limit && state.samples.len() >= state.effective_retention_limit() {
            break;
        }
        match consumer.pop() {
            Ok(sample) => {
                state.samples.push_back(sample);
                popped += 1;
            }
            Err(_) => break,
        }
    }
    popped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_pcm_buffer_samples_matches_retention_cap() {
        assert_eq!(MAX_PCM_BUFFER_SAMPLES, MAX_PCM_RETENTION_SAMPLES);
        assert_eq!(MAX_PCM_RETENTION_SAMPLES, 57_600_000);
    }

    #[test]
    fn drain_consumer_stops_at_retention_limit_without_exceeding() {
        let (mut prod, mut cons) = rtrb::RingBuffer::<f32>::new(10_000);
        let mut state = PcmBufferState::new();
        state.retention_limit_samples = Some(100);

        for i in 0..150 {
            prod.push(i as f32).expect("push");
        }

        let drained = drain_consumer(&mut cons, &mut state, true);
        assert_eq!(drained, 100);
        assert_eq!(state.samples.len(), 100);
        assert_eq!(cons.slots(), 50);
    }

    #[test]
    fn final_drain_can_drain_rtrb_past_limit_for_shutdown_flush() {
        let (mut prod, mut cons) = rtrb::RingBuffer::<f32>::new(200);
        let mut state = PcmBufferState::new();
        state.retention_limit_samples = Some(50);
        state.samples.extend(std::iter::repeat_n(0.0f32, 50));

        for i in 0..30 {
            prod.push(i as f32).expect("push");
        }

        let drained = drain_consumer(&mut cons, &mut state, false);
        assert_eq!(drained, 30);
        assert_eq!(state.samples.len(), 80);
    }
}
