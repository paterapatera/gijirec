//! Downstream PCM chunk bus with bounded backpressure.

use crate::tauri::bounded_bus::{ConsumerDeliverOutcome, flush_registered_consumer_queue};
use crate::tauri::observability;
use gijirec_domain::audio::pcm_chunk::{
    CHUNK_FRAME_COUNT, PcmChunk, PcmChunkConsumer, PcmConsumerError, SAMPLE_RATE_HZ,
};
use std::sync::{Arc, Mutex};

/// Duration of one [`PcmChunk`] at [`SAMPLE_RATE_HZ`] (100 ms).
const CHUNK_DURATION_MS: u64 = CHUNK_FRAME_COUNT as u64 * 1_000 / SAMPLE_RATE_HZ as u64;

/// Backlog headroom while inference runs, aligned with compose rtrb sizing (10×30 s ≈ 5 min).
const BACKLOG_HEADROOM_SECONDS: u64 = 300;

/// Maximum queued chunks before dropping oldest.
///
/// Sized for slow 30 s window inference with continued 100 ms chunk capture (Req 2.1/2.2).
/// Worst case: 5 min / 100 ms = 3000 chunks — must avoid oldest-drop during slow inference.
/// Overflow beyond this still drops oldest (v1 backpressure); [`flush_queue`] Internal errors
/// increment drops separately.
pub const MAX_QUEUED_CHUNKS: usize =
    (BACKLOG_HEADROOM_SECONDS * 1_000 / CHUNK_DURATION_MS) as usize;

/// Delivers [`PcmChunk`] to a single registered downstream consumer.
pub struct PcmChunkBus {
    consumer: Mutex<Option<Arc<dyn PcmChunkConsumer>>>,
    queue: Mutex<Vec<PcmChunk>>,
    drops_total: Mutex<u64>,
}

impl PcmChunkBus {
    pub fn new() -> Self {
        Self {
            consumer: Mutex::new(None),
            queue: Mutex::new(Vec::with_capacity(MAX_QUEUED_CHUNKS)),
            drops_total: Mutex::new(0),
        }
    }

    /// Registers the single v1 downstream consumer.
    pub fn register(&self, consumer: Arc<dyn PcmChunkConsumer>) {
        *self.consumer.lock().expect("lock") = Some(consumer);
        self.flush_queue();
    }

    /// Enqueues a chunk for delivery; drops oldest when over capacity.
    pub fn publish(&self, chunk: PcmChunk) {
        {
            let mut queue = self.queue.lock().expect("lock");
            if queue.len() >= MAX_QUEUED_CHUNKS {
                queue.remove(0);
                let mut drops = self.drops_total.lock().expect("lock");
                *drops += 1;
                observability::log_buffer_drop(*drops);
            }
            queue.push(chunk);
        }
        self.flush_queue();
    }

    pub fn buffer_drops_total(&self) -> u64 {
        *self.drops_total.lock().expect("lock")
    }

    fn flush_queue(&self) {
        flush_registered_consumer_queue(&self.consumer, &self.queue, |consumer, chunk| {
            match consumer.on_pcm_chunk(chunk.clone()) {
                Ok(()) => ConsumerDeliverOutcome::Consumed,
                Err(PcmConsumerError::Disconnected) => ConsumerDeliverOutcome::Stop(chunk),
                Err(PcmConsumerError::Internal(_)) => {
                    let mut drops = self.drops_total.lock().expect("lock");
                    *drops += 1;
                    observability::log_buffer_drop(*drops);
                    ConsumerDeliverOutcome::Consumed
                }
            }
        });
    }
}

impl Default for PcmChunkBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::fixtures::sample_pcm_chunk;
    use std::time::{Duration, Instant};

    struct MockConsumer {
        received: Mutex<Vec<u64>>,
        delay: Duration,
    }

    impl MockConsumer {
        fn immediate() -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                delay: Duration::ZERO,
            }
        }

        fn slow(delay: Duration) -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                delay,
            }
        }

        fn sequences(&self) -> Vec<u64> {
            self.received.lock().expect("lock").clone()
        }
    }

    impl PcmChunkConsumer for MockConsumer {
        fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError> {
            if self.delay > Duration::ZERO {
                std::thread::sleep(self.delay);
            }
            self.received.lock().expect("lock").push(chunk.sequence());
            Ok(())
        }
    }
    // Integration Tests 4: consumer 登録後 100 ms 以内に最初のチャンク到達 (req 2.3)
    #[test]
    fn delivers_first_chunk_within_100ms_after_register() {
        let bus = PcmChunkBus::new();
        bus.publish(sample_pcm_chunk(0));

        let consumer = Arc::new(MockConsumer::immediate());
        let start = Instant::now();
        bus.register(Arc::clone(&consumer) as Arc<dyn PcmChunkConsumer>);

        assert!(
            start.elapsed() < Duration::from_millis(100),
            "delivery should be immediate on register"
        );
        assert_eq!(consumer.sequences(), vec![0]);
    }

    // Integration Tests 5: キュー上限超過時にドロップが記録される (req 2.3, 7.2)
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn max_queued_chunks_covers_worst_case_inference_backlog() {
        const EXPECTED_WORST_CASE_QUEUED_CHUNKS: usize = 3000;
        assert!(
            MAX_QUEUED_CHUNKS >= EXPECTED_WORST_CASE_QUEUED_CHUNKS,
            "queue must hold ~5 min of 100 ms chunks during slow inference (Req 2.1/2.2)"
        );
    }

    #[test]
    fn pre_register_publish_at_capacity_does_not_drop_oldest() {
        let bus = PcmChunkBus::new();
        for seq in 0..MAX_QUEUED_CHUNKS as u64 {
            bus.publish(sample_pcm_chunk(seq));
        }
        assert_eq!(
            bus.buffer_drops_total(),
            0,
            "worst-case depth must not drop oldest chunks before capacity"
        );

        let consumer = Arc::new(MockConsumer::immediate());
        bus.register(Arc::clone(&consumer) as Arc<dyn PcmChunkConsumer>);
        assert_eq!(consumer.sequences().len(), MAX_QUEUED_CHUNKS);
        assert_eq!(bus.buffer_drops_total(), 0);
    }

    #[test]
    fn records_drops_when_queue_exceeds_capacity() {
        let bus = PcmChunkBus::new();
        let overflow = 2usize;
        for seq in 0..(MAX_QUEUED_CHUNKS as u64 + overflow as u64) {
            bus.publish(sample_pcm_chunk(seq));
        }
        assert_eq!(
            bus.buffer_drops_total(),
            overflow as u64,
            "beyond worst-case capacity should drop oldest chunks"
        );
    }

    // Integration Tests 5: 遅延 consumer でもパニックせず配信 (req 2.3, 7.2)
    #[test]
    fn slow_consumer_still_receives_without_panicking() {
        let bus = PcmChunkBus::new();
        let consumer = Arc::new(MockConsumer::slow(Duration::from_millis(10)));
        bus.register(Arc::clone(&consumer) as Arc<dyn PcmChunkConsumer>);

        for seq in 0..3 {
            bus.publish(sample_pcm_chunk(seq));
        }

        assert_eq!(consumer.sequences().len(), 3);
    }

    // Integration Tests 5: consumer 登録前の溢れなしで遅延 consumer が全チャンクを受信
    #[test]
    fn delayed_consumer_receives_all_preregistered_chunks_without_drops() {
        let bus = PcmChunkBus::new();
        let count = 10u64;
        for seq in 0..count {
            bus.publish(sample_pcm_chunk(seq));
        }
        assert_eq!(
            bus.buffer_drops_total(),
            0,
            "pre-register backlog within capacity must not drop oldest chunks"
        );

        let consumer = Arc::new(MockConsumer::slow(Duration::from_millis(5)));
        bus.register(Arc::clone(&consumer) as Arc<dyn PcmChunkConsumer>);

        assert_eq!(consumer.sequences(), (0..count).collect::<Vec<_>>());
        assert_eq!(bus.buffer_drops_total(), 0);
    }
}
