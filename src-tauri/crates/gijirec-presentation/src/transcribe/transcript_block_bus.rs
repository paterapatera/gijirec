//! Downstream TranscriptBlock bus with bounded ring buffer and Tauri event emission.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use gijirec_domain::transcribe::{
    TranscriptBlock, TranscriptBlockConsumer, TranscriptConsumerError,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

/// Maximum retained blocks in memory ring buffer (~500 blocks).
pub const MAX_QUEUED_BLOCKS: usize = 500;

/// Tauri event name for appended transcript blocks.
pub const BLOCK_APPENDED_EVENT: &str = "whisper-transcribe://block-appended";

/// Payload for `whisper-transcribe://block-appended` event.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TranscriptBlockAppendedPayload {
    pub block: TranscriptBlockPayload,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TranscriptBlockPayload {
    pub block_id: String,
    pub sequence: u64,
    pub text: String,
    pub start_timestamp_ms: u64,
    pub language: String,
}

impl From<&TranscriptBlock> for TranscriptBlockPayload {
    fn from(block: &TranscriptBlock) -> Self {
        Self {
            block_id: block.block_id.clone(),
            sequence: block.sequence,
            text: block.text.clone(),
            start_timestamp_ms: block.start_timestamp_ms,
            language: block.language.clone(),
        }
    }
}

/// Abstract emitter trait for block events (enables mock testing without AppHandle).
pub trait TranscriptBlockEventEmitter: Send + Sync {
    fn emit_block_appended(&self, payload: TranscriptBlockAppendedPayload) -> Result<(), String>;
}

/// Tauri implementation of [`TranscriptBlockEventEmitter`].
pub struct TauriTranscriptBlockEventEmitter<R: Runtime = tauri::Wry> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriTranscriptBlockEventEmitter<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> TranscriptBlockEventEmitter for TauriTranscriptBlockEventEmitter<R> {
    fn emit_block_appended(&self, payload: TranscriptBlockAppendedPayload) -> Result<(), String> {
        self.app
            .emit(BLOCK_APPENDED_EVENT, payload)
            .map_err(|e| e.to_string())
    }
}

/// Metrics hook for recording block buffer drops.
pub type BlockDropCallback = Arc<dyn Fn(u64) + Send + Sync>;

/// Clock source for capture-referenced event timestamp.
pub type TimestampClock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Delivers [`TranscriptBlock`] to a single registered downstream consumer and Tauri event stream.
pub struct TranscriptBlockBus {
    consumer: Mutex<Option<Arc<dyn TranscriptBlockConsumer>>>,
    queue: Mutex<Vec<TranscriptBlock>>,
    drops_total: AtomicU64,
    emitter: Mutex<Option<Arc<dyn TranscriptBlockEventEmitter>>>,
    on_drop: Mutex<Option<BlockDropCallback>>,
    clock: Mutex<Option<TimestampClock>>,
}

impl TranscriptBlockBus {
    /// Creates a new `TranscriptBlockBus` without emitter (useful for testing or initial setup).
    pub fn new() -> Self {
        Self {
            consumer: Mutex::new(None),
            queue: Mutex::new(Vec::with_capacity(MAX_QUEUED_BLOCKS)),
            drops_total: AtomicU64::new(0),
            emitter: Mutex::new(None),
            on_drop: Mutex::new(None),
            clock: Mutex::new(None),
        }
    }

    /// Creates a new `TranscriptBlockBus` with a specified event emitter.
    pub fn with_emitter(emitter: Arc<dyn TranscriptBlockEventEmitter>) -> Self {
        Self {
            consumer: Mutex::new(None),
            queue: Mutex::new(Vec::with_capacity(MAX_QUEUED_BLOCKS)),
            drops_total: AtomicU64::new(0),
            emitter: Mutex::new(Some(emitter)),
            on_drop: Mutex::new(None),
            clock: Mutex::new(None),
        }
    }

    /// Sets the Tauri/mock event emitter.
    pub fn set_emitter(&self, emitter: Arc<dyn TranscriptBlockEventEmitter>) {
        if let Ok(mut lock) = self.emitter.lock() {
            *lock = Some(emitter);
        }
    }

    /// Sets a clock function returning capture-relative timestamp in milliseconds.
    pub fn set_clock(&self, clock: TimestampClock) {
        if let Ok(mut lock) = self.clock.lock() {
            *lock = Some(clock);
        }
    }

    /// Sets a callback invoked whenever oldest block is dropped due to queue capacity overflow.
    pub fn set_drop_callback(&self, callback: BlockDropCallback) {
        if let Ok(mut lock) = self.on_drop.lock() {
            *lock = Some(callback);
        }
    }

    /// Registers the single downstream consumer and flushes queued blocks.
    pub fn register(&self, consumer: Arc<dyn TranscriptBlockConsumer>) {
        *self.consumer.lock().expect("lock") = Some(consumer);
        self.flush_queue();
    }

    /// Publishes a new [`TranscriptBlock`] (append-only).
    pub fn publish(&self, block: TranscriptBlock) {
        // 1. Emit Tauri event
        let emitter_guard = self.emitter.lock().ok();
        let emitter_opt = emitter_guard.as_ref().and_then(|g| g.as_ref());
        if let Some(emitter) = emitter_opt {
            let clock_guard = self.clock.lock().ok();
            let timestamp_ms = clock_guard
                .as_ref()
                .and_then(|g| g.as_ref())
                .map(|c| c())
                .unwrap_or(block.start_timestamp_ms);

            let payload = TranscriptBlockAppendedPayload {
                block: TranscriptBlockPayload::from(&block),
                timestamp_ms,
            };
            let _ = emitter.emit_block_appended(payload);
        }

        // 2. Queue in memory buffer (ring buffer of MAX_QUEUED_BLOCKS)
        {
            let mut queue = self.queue.lock().expect("lock");
            self.push_with_drop_handling(&mut queue, block);
        }

        // 3. Deliver to downstream consumer if registered
        self.flush_queue();
    }

    fn push_with_drop_handling(&self, queue: &mut Vec<TranscriptBlock>, block: TranscriptBlock) {
        if queue.len() >= MAX_QUEUED_BLOCKS {
            queue.remove(0);
            let drops = self.drops_total.fetch_add(1, Ordering::Relaxed) + 1;
            if let Ok(guard) = self.on_drop.lock()
                && let Some(ref cb) = *guard
            {
                cb(drops);
            }
        }
        queue.push(block);
    }

    /// Returns total buffer drops.
    pub fn buffer_drops_total(&self) -> u64 {
        self.drops_total.load(Ordering::Relaxed)
    }

    fn flush_queue(&self) {
        let consumer = self.consumer.lock().expect("lock").clone();
        if consumer.is_none() {
            return;
        }
        let consumer = consumer.expect("checked");
        let mut queue = self.queue.lock().expect("lock");
        let mut remaining = Vec::new();
        for block in queue.drain(..) {
            if let Err(_err) = consumer.on_block_appended(block.clone()) {
                remaining.push(block);
            }
        }
        if !remaining.is_empty() {
            *queue = remaining;
        }
    }
}

impl TranscriptBlockConsumer for TranscriptBlockBus {
    fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError> {
        self.publish(block);
        Ok(())
    }
}

impl Default for TranscriptBlockBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingEmitter {
        emitted: Mutex<Vec<TranscriptBlockAppendedPayload>>,
    }

    impl RecordingEmitter {
        fn new() -> Self {
            Self {
                emitted: Mutex::new(Vec::new()),
            }
        }
    }

    impl TranscriptBlockEventEmitter for RecordingEmitter {
        fn emit_block_appended(
            &self,
            payload: TranscriptBlockAppendedPayload,
        ) -> Result<(), String> {
            self.emitted.lock().unwrap().push(payload);
            Ok(())
        }
    }

    struct RecordingConsumer {
        received: Mutex<Vec<TranscriptBlock>>,
        fail: Mutex<bool>,
    }

    impl RecordingConsumer {
        fn new() -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                fail: Mutex::new(false),
            }
        }
    }

    impl TranscriptBlockConsumer for RecordingConsumer {
        fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError> {
            if *self.fail.lock().unwrap() {
                return Err(TranscriptConsumerError::Internal("failed".to_string()));
            }
            self.received.lock().unwrap().push(block);
            Ok(())
        }
    }

    fn test_block(seq: u64, text: &str) -> TranscriptBlock {
        TranscriptBlock::new(
            "12345678-1234-4234-8234-123456789abc".to_string(),
            seq,
            text.to_string(),
            seq * 1000,
            "ja".to_string(),
        )
        .expect("valid block")
    }

    #[test]
    fn emits_tauri_event_with_capture_relative_timestamp_on_publish() {
        let emitter = Arc::new(RecordingEmitter::new());
        let bus = TranscriptBlockBus::with_emitter(emitter.clone());
        bus.set_clock(Arc::new(|| 1250));

        bus.publish(test_block(1, "こんにちは"));

        let events = emitter.emitted.lock().unwrap().clone();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].block.sequence, 1);
        assert_eq!(events[0].block.text, "こんにちは");
        assert_eq!(events[0].block.start_timestamp_ms, 1000);
        assert_eq!(events[0].timestamp_ms, 1250);
    }

    #[test]
    fn defaults_event_timestamp_to_block_start_timestamp_when_clock_unset() {
        let emitter = Arc::new(RecordingEmitter::new());
        let bus = TranscriptBlockBus::with_emitter(emitter.clone());

        bus.publish(test_block(3, "時計未設定"));

        let events = emitter.emitted.lock().unwrap().clone();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].timestamp_ms, 3000);
    }

    #[test]
    fn implements_transcript_block_consumer_for_block_emitter_integration() {
        let emitter = Arc::new(RecordingEmitter::new());
        let bus = Arc::new(TranscriptBlockBus::with_emitter(emitter.clone()));

        let res = bus.on_block_appended(test_block(1, "ブロックエミッター経由"));
        assert!(res.is_ok());

        let events = emitter.emitted.lock().unwrap().clone();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].block.text, "ブロックエミッター経由");
    }

    #[test]
    fn delivers_to_registered_consumer() {
        let bus = TranscriptBlockBus::new();
        let consumer = Arc::new(RecordingConsumer::new());
        bus.register(consumer.clone());

        bus.publish(test_block(1, "発話1"));
        bus.publish(test_block(2, "発話2"));

        let received = consumer.received.lock().unwrap().clone();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].sequence, 1);
        assert_eq!(received[1].sequence, 2);
    }

    #[test]
    fn drops_oldest_block_at_capacity_overflow() {
        let bus = TranscriptBlockBus::new();
        let dropped_counts = Arc::new(Mutex::new(Vec::new()));
        let dc = Arc::clone(&dropped_counts);
        bus.set_drop_callback(Arc::new(move |drops| {
            dc.lock().unwrap().push(drops);
        }));

        for i in 1..=MAX_QUEUED_BLOCKS {
            bus.publish(test_block(i as u64, "block"));
        }
        assert_eq!(bus.buffer_drops_total(), 0);

        // 501th block causes drop of the 1st
        bus.publish(test_block(501, "block 501"));
        assert_eq!(bus.buffer_drops_total(), 1);
        assert_eq!(*dropped_counts.lock().unwrap(), vec![1]);

        // Register consumer now: should receive blocks 2..=501 (500 items)
        let consumer = Arc::new(RecordingConsumer::new());
        bus.register(consumer.clone());

        let received = consumer.received.lock().unwrap().clone();
        assert_eq!(received.len(), MAX_QUEUED_BLOCKS);
        assert_eq!(received.first().unwrap().sequence, 2);
        assert_eq!(received.last().unwrap().sequence, 501);
    }
}
