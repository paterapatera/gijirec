//! PcmIngestConsumer implementing PcmChunkConsumer to ingest PCM into rtrb.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gijirec_domain::audio::pcm_chunk::{PcmChunk, PcmChunkConsumer, PcmConsumerError};

/// Max spin wait for rtrb space before signaling backpressure (bus re-queues chunk).
const RTRB_PUSH_SPIN_BUDGET: Duration = Duration::from_millis(5);

/// Fixed transcribe-path gain targeting ~−18 to −17 dBFS window RMS (v1).
const TRANSCRIBE_INGEST_GAIN: f32 = 1.25;
/// Soft limit ceiling after gain to prevent clipping (matches mixer `SOFT_LIMIT`).
const TRANSCRIBE_SOFT_LIMIT: f32 = 0.95;

fn soft_limit(sample: f32) -> f32 {
    sample.clamp(-TRANSCRIBE_SOFT_LIMIT, TRANSCRIBE_SOFT_LIMIT)
}

fn apply_transcribe_ingest_gain(sample: f32) -> f32 {
    soft_limit(sample * TRANSCRIBE_INGEST_GAIN)
}

/// Metrics hook for recording sequence gaps.
pub type SequenceGapCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Hook invoked with chunk RMS after PCM normalization (e.g. stall watchdog input detection).
pub type PcmChunkRmsCallback = Arc<dyn Fn(f32) + Send + Sync>;

/// Ingests [`PcmChunk`]s from `PcmChunkBus` into an `rtrb::Producer<f32>` with normalized
/// `f32` samples. When the rtrb lacks space, spins briefly then returns
/// [`PcmConsumerError::Disconnected`] so the bus can re-queue the chunk without sample loss.
pub struct PcmIngestConsumer {
    producer: Mutex<rtrb::Producer<f32>>,
    last_sequence: AtomicU64,
    has_seen_first_chunk: AtomicBool,
    sequence_gaps_total: AtomicU64,
    rtrb_overflow_count: Arc<AtomicU64>,
    on_sequence_gap: Option<SequenceGapCallback>,
    on_pcm_rms: Option<PcmChunkRmsCallback>,
}

impl PcmIngestConsumer {
    /// Creates a new `PcmIngestConsumer` connected to an `rtrb::Producer`.
    pub fn new(producer: rtrb::Producer<f32>) -> Self {
        Self {
            producer: Mutex::new(producer),
            last_sequence: AtomicU64::new(0),
            has_seen_first_chunk: AtomicBool::new(false),
            sequence_gaps_total: AtomicU64::new(0),
            rtrb_overflow_count: Arc::new(AtomicU64::new(0)),
            on_sequence_gap: None,
            on_pcm_rms: None,
        }
    }

    /// Sets an optional callback invoked with RMS for each ingested chunk.
    pub fn set_pcm_rms_callback(&mut self, callback: PcmChunkRmsCallback) {
        self.on_pcm_rms = Some(callback);
    }

    /// Sets an optional callback invoked when sequence gaps are detected.
    pub fn set_sequence_gap_callback(&mut self, callback: SequenceGapCallback) {
        self.on_sequence_gap = Some(callback);
    }

    /// Returns the total number of detected sequence gaps.
    pub fn sequence_gaps_total(&self) -> u64 {
        self.sequence_gaps_total.load(Ordering::Relaxed)
    }

    /// Returns how often rtrb push hit backpressure (spin budget exhausted).
    pub fn rtrb_overflow_count(&self) -> u64 {
        self.rtrb_overflow_count.load(Ordering::Relaxed)
    }

    /// Shared counter for wiring into batch observability on the worker.
    pub fn rtrb_overflow_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.rtrb_overflow_count)
    }

    /// Resets sequence tracking (e.g., at new capture session boundary).
    pub fn reset_sequence_tracking(&self) {
        self.has_seen_first_chunk.store(false, Ordering::Relaxed);
        self.last_sequence.store(0, Ordering::Relaxed);
    }

    fn check_sequence_gap(&self, last: u64, seq: u64) {
        if seq > last + 1 {
            let gap_count = seq - (last + 1);
            self.sequence_gaps_total
                .fetch_add(gap_count, Ordering::Relaxed);
            if let Some(ref cb) = self.on_sequence_gap {
                cb(last, seq);
            }
        }
    }

    /// Pushes normalized samples atomically; spins briefly for space, then signals backpressure.
    #[allow(clippy::excessive_nesting)]
    fn push_normalized_samples(
        producer: &mut rtrb::Producer<f32>,
        samples: &[f32],
    ) -> Result<(), PcmConsumerError> {
        let deadline = Instant::now() + RTRB_PUSH_SPIN_BUDGET;
        loop {
            match producer.push_entire_slice(samples) {
                Ok(()) => return Ok(()),
                Err(rtrb::chunks::ChunkError::TooFewSlots(_)) => {
                    if Instant::now() >= deadline {
                        // Disconnected lets PcmChunkBus re-queue the chunk without counting a drop.
                        return Err(PcmConsumerError::Disconnected);
                    }
                    std::thread::yield_now();
                }
            }
        }
    }
}

impl PcmChunkConsumer for PcmIngestConsumer {
    fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError> {
        let seq = chunk.sequence();

        // Detect sequence gaps
        if self.has_seen_first_chunk.swap(true, Ordering::AcqRel) {
            let last = self.last_sequence.load(Ordering::Acquire);
            self.check_sequence_gap(last, seq);
        }
        self.last_sequence.store(seq, Ordering::Release);

        // Convert i16 samples to normalized f32 and push into rtrb non-blockingly
        let mut producer = self
            .producer
            .lock()
            .map_err(|e| PcmConsumerError::Internal(format!("poisoned producer lock: {e}")))?;

        let samples = chunk.samples();
        let mut normalized = Vec::with_capacity(samples.len());
        let mut sum_sq = 0.0f32;
        for &sample in samples {
            let value = apply_transcribe_ingest_gain((sample as f32) / 32768.0);
            sum_sq += value * value;
            normalized.push(value);
        }

        Self::push_normalized_samples(&mut producer, &normalized).map_err(|err| {
            if matches!(err, PcmConsumerError::Disconnected) {
                self.rtrb_overflow_count.fetch_add(1, Ordering::Relaxed);
            }
            err
        })?;

        if let Some(ref cb) = self.on_pcm_rms
            && !samples.is_empty()
        {
            cb((sum_sq / samples.len() as f32).sqrt());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::pcm_chunk::CHUNK_FRAME_COUNT;

    fn make_test_chunk(sequence: u64, val: i16) -> PcmChunk {
        let samples = vec![val; CHUNK_FRAME_COUNT as usize];
        PcmChunk::new(sequence, samples, sequence * 100).expect("valid chunk")
    }

    #[test]
    fn on_pcm_chunk_returns_immediately_non_blocking() {
        let (prod, cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        let start = std::time::Instant::now();
        let res = consumer.on_pcm_chunk(make_test_chunk(1, 100));
        let elapsed = start.elapsed();

        assert!(res.is_ok());
        assert!(
            elapsed < std::time::Duration::from_millis(5),
            "on_pcm_chunk must return immediately without blocking I/O: {:?}",
            elapsed
        );
        assert_eq!(cons.slots(), CHUNK_FRAME_COUNT as usize);
    }

    #[test]
    fn ingests_chunk_samples_into_rtrb_normalized() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        let chunk = make_test_chunk(1, 16384); // 16384 / 32768.0 = 0.5
        let res = consumer.on_pcm_chunk(chunk);
        assert!(res.is_ok());

        assert_eq!(cons.slots(), CHUNK_FRAME_COUNT as usize);
        let first = cons.pop().expect("pop");
        let expected = apply_transcribe_ingest_gain(0.5);
        assert!((first - expected).abs() < 1e-4);
    }

    #[test]
    fn detects_sequence_gaps_and_increments_metric() {
        let (prod, _cons) = rtrb::RingBuffer::<f32>::new(8192);
        let mut consumer = PcmIngestConsumer::new(prod);

        let gap_detected = Arc::new(Mutex::new(Vec::new()));
        let gap_clone = Arc::clone(&gap_detected);
        consumer.set_sequence_gap_callback(Arc::new(move |from, to| {
            gap_clone.lock().unwrap().push((from, to));
        }));

        assert_eq!(consumer.sequence_gaps_total(), 0);

        // Sequence 1
        consumer.on_pcm_chunk(make_test_chunk(1, 0)).unwrap();
        assert_eq!(consumer.sequence_gaps_total(), 0);

        // Sequence 2 (no gap)
        consumer.on_pcm_chunk(make_test_chunk(2, 0)).unwrap();
        assert_eq!(consumer.sequence_gaps_total(), 0);

        // Sequence 5 (gap of 2: missed 3, 4)
        consumer.on_pcm_chunk(make_test_chunk(5, 0)).unwrap();
        assert_eq!(consumer.sequence_gaps_total(), 2);

        let gaps = gap_detected.lock().unwrap().clone();
        assert_eq!(gaps, vec![(2, 5)]);
    }

    #[test]
    fn rtrb_full_returns_disconnected_not_internal_without_partial_push() {
        let (prod, cons) = rtrb::RingBuffer::<f32>::new(100);
        let consumer = PcmIngestConsumer::new(prod);

        let res = consumer.on_pcm_chunk(make_test_chunk(1, 0));
        assert!(
            matches!(res, Err(PcmConsumerError::Disconnected)),
            "full rtrb must signal backpressure, not Internal: {res:?}"
        );
        assert_eq!(
            cons.slots(),
            0,
            "chunk must not be partially written when backpressured"
        );
        assert_eq!(consumer.rtrb_overflow_count(), 1);
    }

    #[test]
    fn ingests_all_samples_after_transient_rtrb_pressure() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(2400);
        let consumer = PcmIngestConsumer::new(prod);

        consumer
            .on_pcm_chunk(make_test_chunk(1, 100))
            .expect("first chunk fits");
        assert!(matches!(
            consumer.on_pcm_chunk(make_test_chunk(2, 200)),
            Err(PcmConsumerError::Disconnected)
        ));
        assert_eq!(cons.slots(), CHUNK_FRAME_COUNT as usize);

        while cons.pop().is_ok() {}

        consumer
            .on_pcm_chunk(make_test_chunk(3, 300))
            .expect("retry after drain");
        assert_eq!(cons.slots(), CHUNK_FRAME_COUNT as usize);
        let first = cons.pop().expect("pop");
        let expected = apply_transcribe_ingest_gain(300.0 / 32768.0);
        assert!((first - expected).abs() < 1e-4);
    }

    #[test]
    fn reset_sequence_tracking_allows_new_session_from_low_sequence() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(8192);
        let consumer = PcmIngestConsumer::new(prod);

        consumer.on_pcm_chunk(make_test_chunk(100, 0)).unwrap();
        // Reset for new session
        consumer.reset_sequence_tracking();
        // Drain ring buffer
        while cons.pop().is_ok() {}

        // Chunk with seq 1 should not be considered a gap from 100
        consumer.on_pcm_chunk(make_test_chunk(1, 0)).unwrap();
        assert_eq!(consumer.sequence_gaps_total(), 0);
    }

    fn chunk_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|sample| sample * sample).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    fn constant_amplitude_chunk(sequence: u64, amplitude: f32) -> PcmChunk {
        let sample = (amplitude * 32768.0).round() as i16;
        make_test_chunk(sequence, sample)
    }

    #[test]
    fn silence_stays_below_silence_threshold_after_gain() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        // Pre-gain RMS well below SILENCE_RMS_THRESHOLD (0.008).
        consumer
            .on_pcm_chunk(constant_amplitude_chunk(1, 0.004))
            .expect("ingest");

        let mut gained = Vec::with_capacity(CHUNK_FRAME_COUNT as usize);
        while cons.slots() > 0 {
            gained.push(cons.pop().expect("pop"));
        }

        assert!(
            chunk_rms(&gained) < 0.008,
            "quiet input must remain below silence skip threshold after gain: {}",
            chunk_rms(&gained)
        );
    }

    #[test]
    fn nominal_input_reaches_target_rms_after_gain() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        // ~−20 dBFS pre-gain; ×1.25 lands near −18 to −17 dBFS target.
        consumer
            .on_pcm_chunk(constant_amplitude_chunk(1, 0.10))
            .expect("ingest");

        let mut gained = Vec::with_capacity(CHUNK_FRAME_COUNT as usize);
        while cons.slots() > 0 {
            gained.push(cons.pop().expect("pop"));
        }

        let rms = chunk_rms(&gained);
        assert!(
            (0.12..=0.13).contains(&rms),
            "expected post-gain RMS in 0.12..=0.13, got {rms}"
        );
    }

    #[test]
    fn soft_limit_caps_high_peak_input() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        consumer
            .on_pcm_chunk(constant_amplitude_chunk(1, 0.9))
            .expect("ingest");

        while cons.slots() > 0 {
            let sample = cons.pop().expect("pop");
            assert!(
                sample.abs() <= TRANSCRIBE_SOFT_LIMIT + f32::EPSILON,
                "sample {sample} exceeded soft limit {TRANSCRIBE_SOFT_LIMIT}"
            );
        }
    }
}
