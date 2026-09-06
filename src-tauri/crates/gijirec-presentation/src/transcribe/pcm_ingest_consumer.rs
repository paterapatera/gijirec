//! PcmIngestConsumer implementing PcmChunkConsumer to ingest PCM into rtrb.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gijirec_domain::audio::pcm_chunk::{PcmChunk, PcmChunkConsumer, PcmConsumerError};

/// Metrics hook for recording sequence gaps.
pub type SequenceGapCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Hook invoked with chunk RMS after PCM normalization (e.g. stall watchdog input detection).
pub type PcmChunkRmsCallback = Arc<dyn Fn(f32) + Send + Sync>;

/// Ingests [`PcmChunk`]s from `PcmChunkBus` directly into an `rtrb::Producer<f32>`
/// with non-blocking conversion to `f32` in `[-1.0, 1.0]`.
pub struct PcmIngestConsumer {
    producer: Mutex<rtrb::Producer<f32>>,
    last_sequence: AtomicU64,
    has_seen_first_chunk: AtomicBool,
    sequence_gaps_total: AtomicU64,
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
        let mut sum_sq = 0.0f32;
        for &sample in samples {
            // Normalize i16 (-32768..=32767) to f32 (-1.0..=1.0)
            let normalized = (sample as f32) / 32768.0;
            sum_sq += normalized * normalized;
            if let Err(rtrb::PushError::Full(_)) = producer.push(normalized) {
                // When rtrb is full, return error so bus knows chunks were dropped or backpressured
                return Err(PcmConsumerError::Internal("rtrb buffer full".to_string()));
            }
        }

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
        assert!((first - 0.5).abs() < 1e-4);
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
    fn returns_internal_error_when_rtrb_is_full() {
        let (prod, _cons) = rtrb::RingBuffer::<f32>::new(100); // smaller than 1600 frame count
        let consumer = PcmIngestConsumer::new(prod);

        let res = consumer.on_pcm_chunk(make_test_chunk(1, 0));
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), PcmConsumerError::Internal(_)));
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
}
