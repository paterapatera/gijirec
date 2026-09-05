//! Downstream PCM chunk bus with bounded backpressure.

use crate::tauri::observability;
use gijirec_domain::audio::pcm_chunk::{PcmChunk, PcmChunkConsumer, PcmConsumerError};
use std::sync::{Arc, Mutex};

/// Maximum queued chunks before dropping (~300 ms at 100 ms/chunk).
pub const MAX_QUEUED_CHUNKS: usize = 3;

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
        let consumer = self.consumer.lock().expect("lock").clone();
        if consumer.is_none() {
            return;
        }
        let consumer = consumer.expect("checked");
        let mut queue = self.queue.lock().expect("lock");
        let mut remaining = Vec::new();
        for chunk in queue.drain(..) {
            match consumer.on_pcm_chunk(chunk.clone()) {
                Ok(()) => {}
                Err(PcmConsumerError::Disconnected) => {
                    remaining.push(chunk);
                    break;
                }
                Err(PcmConsumerError::Internal(_)) => {
                    let mut drops = self.drops_total.lock().expect("lock");
                    *drops += 1;
                    observability::log_buffer_drop(*drops);
                }
            }
        }
        *queue = remaining;
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
    use gijirec_domain::audio::pcm_chunk::CHUNK_FRAME_COUNT;
    use std::time::{Duration, Instant};

    fn sample_chunk(sequence: u64) -> PcmChunk {
        PcmChunk::new(
            sequence,
            vec![0_i16; CHUNK_FRAME_COUNT as usize],
            sequence * 100,
        )
        .expect("chunk")
    }

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
        bus.publish(sample_chunk(0));

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
    fn records_drops_when_queue_exceeds_capacity() {
        let bus = PcmChunkBus::new();
        for seq in 0..5 {
            bus.publish(sample_chunk(seq));
        }
        assert_eq!(
            bus.buffer_drops_total(),
            2,
            "publishing 5 chunks with max 3 should drop 2"
        );
    }

    // Integration Tests 5: 遅延 consumer でもパニックせず配信 (req 2.3, 7.2)
    #[test]
    fn slow_consumer_still_receives_without_panicking() {
        let bus = PcmChunkBus::new();
        let consumer = Arc::new(MockConsumer::slow(Duration::from_millis(10)));
        bus.register(Arc::clone(&consumer) as Arc<dyn PcmChunkConsumer>);

        for seq in 0..3 {
            bus.publish(sample_chunk(seq));
        }

        assert_eq!(consumer.sequences().len(), 3);
    }

    // Integration Tests 5: consumer 登録前の溢れでドロップ記録し、遅延 consumer が残りを受信
    #[test]
    fn delayed_consumer_receives_remainder_after_preregister_overflow_drops() {
        let bus = PcmChunkBus::new();
        for seq in 0..5 {
            bus.publish(sample_chunk(seq));
        }
        assert_eq!(
            bus.buffer_drops_total(),
            2,
            "pre-register overflow should drop oldest chunks"
        );

        let consumer = Arc::new(MockConsumer::slow(Duration::from_millis(5)));
        bus.register(Arc::clone(&consumer) as Arc<dyn PcmChunkConsumer>);

        assert_eq!(consumer.sequences(), vec![2, 3, 4]);
        assert_eq!(bus.buffer_drops_total(), 2);
    }
}
