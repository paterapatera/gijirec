//! 16 kHz mono resampler using rubato on a dedicated thread.

use rubato::{FftFixedIn, Resampler};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Target PCM sample rate for downstream processing.
pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;

/// Rubato chunk size for fixed FFT resampling.
const RESAMPLER_CHUNK_SIZE: usize = 512;

/// Synchronous mono resampler for tests and the worker thread.
pub struct MonoResampler {
    resampler: FftFixedIn<f32>,
    channels: usize,
    pending: Vec<f32>,
}

impl MonoResampler {
    /// Creates a resampler from `source_rate_hz` with `channels` interleaved input.
    pub fn new(
        source_rate_hz: u32,
        channels: usize,
    ) -> Result<Self, rubato::ResamplerConstructionError> {
        let _ = channels;
        let resampler = FftFixedIn::<f32>::new(
            source_rate_hz as usize,
            TARGET_SAMPLE_RATE_HZ as usize,
            RESAMPLER_CHUNK_SIZE,
            1,
            1,
        )?;
        Ok(Self {
            resampler,
            channels,
            pending: Vec::new(),
        })
    }

    /// Resamples interleaved f32 input to 16 kHz mono.
    pub fn process_interleaved(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }

        self.pending.extend_from_slice(input);
        let mut output = Vec::new();

        while self.pending.len() >= RESAMPLER_CHUNK_SIZE * self.channels {
            let chunk: Vec<f32> = self
                .pending
                .drain(..RESAMPLER_CHUNK_SIZE * self.channels)
                .collect();
            if let Some(mono_out) = self.process_chunk(&chunk) {
                output.extend_from_slice(&mono_out);
            } else {
                break;
            }
        }

        output
    }

    fn process_chunk(&mut self, chunk: &[f32]) -> Option<Vec<f32>> {
        let mono = downmix_to_mono(chunk, self.channels);
        match self.resampler.process(&[mono], None) {
            Ok(waves_out) => waves_out.into_iter().next(),
            Err(err) => {
                debug_assert!(false, "resampler process failed: {err:?}");
                None
            }
        }
    }

    /// Drains any remaining samples after the input stream ends.
    pub fn flush(&mut self) -> Vec<f32> {
        if self.pending.is_empty() {
            return Vec::new();
        }

        let pending_frames = self.pending.len() / self.channels;
        let pad_frames = RESAMPLER_CHUNK_SIZE.saturating_sub(pending_frames % RESAMPLER_CHUNK_SIZE);
        if pad_frames > 0 && pad_frames < RESAMPLER_CHUNK_SIZE {
            self.pending
                .resize(self.pending.len() + pad_frames * self.channels, 0.0);
        }
        let drained = std::mem::take(&mut self.pending);
        self.process_interleaved(&drained)
    }
}

/// Producer side for feeding raw interleaved samples into the resampler thread.
pub struct ResamplerInput {
    sender: Sender<Vec<f32>>,
}

impl ResamplerInput {
    pub fn push_interleaved(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let _ = self.sender.send(samples.to_vec());
    }
}

/// Consumer side for 16 kHz mono resampled samples.
pub struct ResampledSampleConsumer {
    receiver: Receiver<Vec<f32>>,
    pending: Vec<f32>,
    cursor: usize,
}

impl ResampledSampleConsumer {
    pub fn pop(&mut self) -> Option<f32> {
        if self.cursor < self.pending.len() {
            let sample = self.pending[self.cursor];
            self.cursor += 1;
            if self.cursor == self.pending.len() {
                self.pending.clear();
                self.cursor = 0;
            }
            return Some(sample);
        }

        match self.receiver.try_recv() {
            Ok(mut chunk) => {
                if chunk.is_empty() {
                    return None;
                }
                let sample = chunk[0];
                if chunk.len() > 1 {
                    self.pending = chunk.split_off(1);
                    self.cursor = 0;
                }
                Some(sample)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn drain_into(&mut self, out: &mut [f32]) -> usize {
        super::f32_ring_consumer::drain_f32_slots(|| self.pop(), out)
    }
}

/// Dedicated-thread resampler pipeline isolated from RT callbacks.
pub struct MonoResamplerPipeline {
    input: ResamplerInput,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct ResamplerThreadContext {
    source_rate_hz: u32,
    channels: usize,
    input_rx: Receiver<Vec<f32>>,
    output_tx: Sender<Vec<f32>>,
    stop: Arc<AtomicBool>,
}

impl MonoResamplerPipeline {
    pub fn spawn(source_rate_hz: u32, channels: usize) -> (Self, ResampledSampleConsumer) {
        let (input_tx, input_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = ResamplerThreadContext {
            source_rate_hz,
            channels,
            input_rx,
            output_tx,
            stop: Arc::clone(&stop),
        };

        let thread = thread::Builder::new()
            .name("mono-resampler".into())
            .spawn(move || resampler_thread(ctx))
            .expect("spawn resampler thread");

        let pipeline = Self {
            input: ResamplerInput { sender: input_tx },
            stop,
            thread: Some(thread),
        };
        let consumer = ResampledSampleConsumer {
            receiver: output_rx,
            pending: Vec::new(),
            cursor: 0,
        };
        (pipeline, consumer)
    }

    pub fn input(&self) -> &ResamplerInput {
        &self.input
    }
}

impl Drop for MonoResamplerPipeline {
    fn drop(&mut self) {
        crate::thread_lifecycle::signal_stop_and_join_thread(&self.stop, &mut self.thread);
    }
}

fn resampler_thread(ctx: ResamplerThreadContext) {
    let mut resampler = match MonoResampler::new(ctx.source_rate_hz, ctx.channels) {
        Ok(r) => r,
        Err(_) => return,
    };

    while !ctx.stop.load(Ordering::SeqCst) {
        match ctx.input_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(chunk) => {
                let out = resampler.process_interleaved(&chunk);
                if !out.is_empty() {
                    let _ = ctx.output_tx.send(out);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let tail = resampler.flush();
    if !tail.is_empty() {
        let _ = ctx.output_tx.send(tail);
    }
}

fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }

    let frames = interleaved.len() / channels;
    let mut mono = Vec::with_capacity(frames);
    for frame in 0..frames {
        let base = frame * channels;
        let mut sum = 0.0_f32;
        for ch in 0..channels {
            sum += interleaved[base + ch];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_48khz_stereo_to_16khz_mono() {
        let source_rate = 48_000_u32;
        let duration_ms = 100_u32;
        let input_frames = source_rate * duration_ms / 1_000;
        let expected_output_frames = TARGET_SAMPLE_RATE_HZ * duration_ms / 1_000;

        let mut input = Vec::with_capacity(input_frames as usize * 2);
        for frame in 0..input_frames {
            let t = frame as f32 / source_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            input.push(sample);
            input.push(sample * 0.5);
        }

        let mut resampler = MonoResampler::new(source_rate, 2).expect("resampler");
        let mut output = resampler.process_interleaved(&input);
        output.extend(resampler.flush());

        let tolerance = (expected_output_frames as f32 * 0.05).ceil() as usize;
        let diff = output.len().abs_diff(expected_output_frames as usize);
        assert!(
            diff <= tolerance,
            "expected ~{expected_output_frames} samples, got {} (diff {diff})",
            output.len()
        );
        assert!(output.iter().any(|s| s.abs() > 0.0));
    }

    #[test]
    fn pipeline_runs_on_dedicated_thread() {
        let (pipeline, mut consumer) = MonoResamplerPipeline::spawn(48_000, 1);
        let input = vec![0.25_f32; 4_800];
        pipeline.input().push_interleaved(&input);
        std::thread::sleep(Duration::from_millis(50));

        let mut drained = [0.0_f32; 512];
        let count = consumer.drain_into(&mut drained);
        assert!(
            count > 0,
            "expected resampled samples from dedicated thread"
        );
    }
}
