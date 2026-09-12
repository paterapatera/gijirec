use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::pcm_buffer::{PcmBufferState, drain_front_samples};

/// Longest PCM handed to whisper.cpp in one batch inference (30 s @ 16 kHz).
pub(crate) const MAX_INFERENCE_WINDOW_SAMPLES: usize = 480_000;

/// Fixed delay between completed batch inference cycles (30 s in production).
#[cfg(not(test))]
pub(crate) const BATCH_INTERVAL: Duration = Duration::from_secs(30);

/// Shorter interval for unit/integration tests that exercise batch timing.
#[cfg(test)]
pub(crate) const BATCH_INTERVAL: Duration = Duration::from_millis(100);

/// Returns whether enough PCM has accumulated for a full 30 s inference window.
pub(crate) fn full_batch_window_ready(unprocessed_samples: usize) -> bool {
    unprocessed_samples >= MAX_INFERENCE_WINDOW_SAMPLES
}

/// Returns whether the first batch cycle should start (transcribing just began).
pub(crate) fn first_cycle_ready(_transcribing_start: Instant, unprocessed_samples: usize) -> bool {
    full_batch_window_ready(unprocessed_samples)
}

/// Returns whether a subsequent batch cycle should start after the previous one completed.
///
/// Inference requires a full 30 s window. When the previous cycle left another full window
/// in the backlog (`backlog_after_last_cycle`), the 30 s interval is skipped so catch-up
/// cycles run back-to-back. Otherwise the worker waits for another full window and the
/// batch interval since the previous cycle completed.
pub(crate) fn next_cycle_ready(
    last_cycle_complete: Instant,
    unprocessed_samples: usize,
    backlog_after_last_cycle: bool,
) -> bool {
    if !full_batch_window_ready(unprocessed_samples) {
        return false;
    }
    backlog_after_last_cycle || last_cycle_complete.elapsed() >= BATCH_INTERVAL
}

pub(crate) fn take_batch_window(
    pcm_buffer: &Arc<Mutex<PcmBufferState>>,
) -> Option<(Vec<f32>, u64)> {
    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    take_batch_window_from_state(&mut state)
}

/// Cuts the next batch window from the buffer: up to [`MAX_INFERENCE_WINDOW_SAMPLES`]
/// from the front, with no VAD or leading-silence trimming.
pub(crate) fn take_batch_window_from_state(state: &mut PcmBufferState) -> Option<(Vec<f32>, u64)> {
    if state.samples.is_empty() {
        return None;
    }
    let cut = state.samples.len().min(MAX_INFERENCE_WINDOW_SAMPLES);
    Some(drain_front_samples(state, cut))
}
