//! PcmIngestConsumer implementing PcmChunkConsumer to ingest PCM into rtrb.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gijirec_domain::audio::pcm_chunk::{PcmChunk, PcmChunkConsumer, PcmConsumerError};

/// Max spin wait for rtrb space before signaling backpressure (bus re-queues chunk).
const RTRB_PUSH_SPIN_BUDGET: Duration = Duration::from_millis(5);

/// Default ingest gain multiplier (~−18 to −17 dBFS window RMS; `transcribe-volume-normalize` equivalent).
const DEFAULT_INGEST_GAIN_MULTIPLIER: f32 = 1.25;
/// Minimum session ingest gain multiplier.
const MIN_INGEST_GAIN_MULTIPLIER: f32 = 0.25;
/// Maximum session ingest gain multiplier.
const MAX_INGEST_GAIN_MULTIPLIER: f32 = 4.0;
/// Soft limit ceiling after gain to prevent clipping (matches mixer `SOFT_LIMIT`).
const TRANSCRIBE_SOFT_LIMIT: f32 = 0.95;

fn soft_limit(sample: f32) -> f32 {
    sample.clamp(-TRANSCRIBE_SOFT_LIMIT, TRANSCRIBE_SOFT_LIMIT)
}

fn apply_ingest_gain(sample: f32, multiplier: f32) -> f32 {
    soft_limit(sample * multiplier)
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
    ingest_gain_multiplier: AtomicU32,
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
            ingest_gain_multiplier: AtomicU32::new(DEFAULT_INGEST_GAIN_MULTIPLIER.to_bits()),
            last_sequence: AtomicU64::new(0),
            has_seen_first_chunk: AtomicBool::new(false),
            sequence_gaps_total: AtomicU64::new(0),
            rtrb_overflow_count: Arc::new(AtomicU64::new(0)),
            on_sequence_gap: None,
            on_pcm_rms: None,
        }
    }

    /// Sets the ingest gain multiplier (clamped to 0.25–4.0). Applied from the next PCM chunk.
    pub fn set_ingest_gain_multiplier(&self, gain: f32) {
        if !gain.is_finite() {
            return;
        }
        let clamped = gain.clamp(MIN_INGEST_GAIN_MULTIPLIER, MAX_INGEST_GAIN_MULTIPLIER);
        self.ingest_gain_multiplier
            .store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Returns the current ingest gain multiplier.
    pub fn ingest_gain_multiplier(&self) -> f32 {
        f32::from_bits(self.ingest_gain_multiplier.load(Ordering::Relaxed))
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
    fn push_normalized_samples(
        producer: &mut rtrb::Producer<f32>,
        samples: &[f32],
    ) -> Result<(), PcmConsumerError> {
        let deadline = Instant::now() + RTRB_PUSH_SPIN_BUDGET;
        while Instant::now() < deadline {
            if producer.push_entire_slice(samples).is_ok() {
                return Ok(());
            }
            std::thread::yield_now();
        }
        // Disconnected lets PcmChunkBus re-queue the chunk without counting a drop.
        Err(PcmConsumerError::Disconnected)
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

        let gain = self.ingest_gain_multiplier();
        let samples = chunk.samples();
        let mut normalized = Vec::with_capacity(samples.len());
        let mut sum_sq = 0.0f32;
        for &sample in samples {
            let value = apply_ingest_gain((sample as f32) / 32768.0, gain);
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
        let expected = apply_ingest_gain(0.5, DEFAULT_INGEST_GAIN_MULTIPLIER);
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
        let expected = apply_ingest_gain(300.0 / 32768.0, DEFAULT_INGEST_GAIN_MULTIPLIER);
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

    fn drain_gained_samples(cons: &mut rtrb::Consumer<f32>) -> Vec<f32> {
        let mut gained = Vec::with_capacity(CHUNK_FRAME_COUNT as usize);
        while cons.slots() > 0 {
            gained.push(cons.pop().expect("pop"));
        }
        gained
    }

    fn ingest_and_drain(
        consumer: &PcmIngestConsumer,
        cons: &mut rtrb::Consumer<f32>,
        chunk: PcmChunk,
    ) -> Vec<f32> {
        consumer.on_pcm_chunk(chunk).expect("ingest");
        drain_gained_samples(cons)
    }

    #[test]
    fn silence_stays_below_silence_threshold_after_gain() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        // Pre-gain RMS well below SILENCE_RMS_THRESHOLD (0.008).
        let gained = ingest_and_drain(&consumer, &mut cons, constant_amplitude_chunk(1, 0.004));

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
        let gained = ingest_and_drain(&consumer, &mut cons, constant_amplitude_chunk(1, 0.10));

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

    #[test]
    fn default_ingest_gain_multiplier_is_transcribe_volume_normalize_equivalent() {
        let (prod, _cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        assert!((consumer.ingest_gain_multiplier() - 1.25).abs() < f32::EPSILON);
    }

    #[test]
    fn set_ingest_gain_multiplier_applies_from_next_chunk() {
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(8192);
        let consumer = PcmIngestConsumer::new(prod);

        consumer
            .on_pcm_chunk(constant_amplitude_chunk(1, 0.10))
            .expect("first chunk at default gain");
        while cons.pop().is_ok() {}

        consumer.set_ingest_gain_multiplier(2.0);
        let gained = ingest_and_drain(&consumer, &mut cons, constant_amplitude_chunk(2, 0.10));

        let rms = chunk_rms(&gained);
        let expected = apply_ingest_gain(0.10, 2.0);
        assert!(
            (rms - expected).abs() < 1e-4,
            "expected post-gain RMS {expected}, got {rms}"
        );
    }

    #[test]
    fn set_ingest_gain_multiplier_clamps_to_valid_range() {
        let (prod, _cons) = rtrb::RingBuffer::<f32>::new(4096);
        let consumer = PcmIngestConsumer::new(prod);

        consumer.set_ingest_gain_multiplier(0.1);
        assert!(
            (consumer.ingest_gain_multiplier() - MIN_INGEST_GAIN_MULTIPLIER).abs() < f32::EPSILON
        );

        consumer.set_ingest_gain_multiplier(10.0);
        assert!(
            (consumer.ingest_gain_multiplier() - MAX_INGEST_GAIN_MULTIPLIER).abs() < f32::EPSILON
        );
    }

    #[test]
    fn on_pcm_rms_uses_post_gain_signal() {
        let (prod, _cons) = rtrb::RingBuffer::<f32>::new(4096);
        let mut consumer = PcmIngestConsumer::new(prod);

        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_clone = Arc::clone(&observed);
        consumer.set_pcm_rms_callback(Arc::new(move |rms| {
            observed_clone.lock().unwrap().push(rms);
        }));

        consumer.set_ingest_gain_multiplier(2.0);
        consumer
            .on_pcm_chunk(constant_amplitude_chunk(1, 0.10))
            .expect("ingest");

        let rms_values = observed.lock().unwrap();
        assert_eq!(rms_values.len(), 1);
        let expected = apply_ingest_gain(0.10, 2.0);
        assert!(
            (rms_values[0] - expected).abs() < 1e-4,
            "on_pcm_rms must reflect post-gain RMS: expected {expected}, got {}",
            rms_values[0]
        );
    }
}
