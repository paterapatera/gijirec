use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Maximum samples retained while waiting for the next inference window (10 min @ 16 kHz).
/// Large enough to hold backlog when inference falls behind without dropping audio.
pub const MAX_PCM_BUFFER_SAMPLES: usize = 9_600_000;

pub(crate) struct PcmBufferState {
    pub(crate) samples: VecDeque<f32>,
    pub(crate) samples_before_buffer: u64,
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

pub(crate) fn drain_pcm_loop(
    mut consumer: rtrb::Consumer<f32>,
    pcm_buffer: Arc<Mutex<PcmBufferState>>,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        let popped = {
            let mut state = pcm_buffer.lock().expect("pcm buffer lock");
            drain_consumer(&mut consumer, &mut state)
        };
        if popped == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }

    let mut state = pcm_buffer.lock().expect("pcm buffer lock");
    drain_consumer(&mut consumer, &mut state);
}

pub(crate) fn drain_front_samples(state: &mut PcmBufferState, cut: usize) -> (Vec<f32>, u64) {
    let base_samples = state.samples_before_buffer;
    let pcm: Vec<f32> = state.samples.drain(..cut).collect();
    state.samples_before_buffer += cut as u64;
    (pcm, base_samples)
}

pub(crate) fn drain_consumer(
    consumer: &mut rtrb::Consumer<f32>,
    state: &mut PcmBufferState,
) -> usize {
    let mut popped = 0usize;
    while let Ok(sample) = consumer.pop() {
        state.samples.push_back(sample);
        popped += 1;
    }
    popped
}
