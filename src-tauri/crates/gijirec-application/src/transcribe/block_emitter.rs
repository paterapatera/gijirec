//! Converts whisper inference segments into downstream [`TranscriptBlock`] values.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use gijirec_domain::transcribe::{
    TranscribeError, TranscriptBlock, TranscriptBlockConsumer, TranscriptBlockError,
    TranscriptConsumerError, TranscriptSegmentSink,
};

/// Emits contract-valid transcript blocks from inference worker segments.
pub struct BlockEmitter<C> {
    consumer: Arc<C>,
    sequence: AtomicU64,
}

impl<C: TranscriptBlockConsumer> BlockEmitter<C> {
    pub fn new(consumer: Arc<C>) -> Self {
        Self {
            consumer,
            sequence: AtomicU64::new(0),
        }
    }

    pub fn next_sequence(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }
}

impl<C: TranscriptBlockConsumer + 'static> TranscriptSegmentSink for BlockEmitter<C> {
    fn on_segment(&self, text: &str, start_ms: u64, language: &str) -> Result<(), TranscribeError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let block_id = uuid::Uuid::new_v4().to_string();
        let block = TranscriptBlock::new(
            block_id,
            sequence,
            trimmed.to_string(),
            start_ms,
            language.to_string(),
        )
        .map_err(map_block_error)?;

        self.consumer
            .on_block_appended(block)
            .map_err(map_consumer_error)
    }
}

fn map_block_error(err: TranscriptBlockError) -> TranscribeError {
    TranscribeError::Internal {
        detail: err.to_string(),
    }
}

fn map_consumer_error(err: TranscriptConsumerError) -> TranscribeError {
    match err {
        TranscriptConsumerError::Closed => TranscribeError::Internal {
            detail: "transcript block consumer closed".to_string(),
        },
        TranscriptConsumerError::Internal(message) => TranscribeError::Internal { detail: message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct RecordingConsumer {
        blocks: Arc<Mutex<Vec<TranscriptBlock>>>,
        fail_with: Arc<Mutex<Option<TranscriptConsumerError>>>,
    }

    impl RecordingConsumer {
        fn new() -> (Self, Arc<Mutex<Vec<TranscriptBlock>>>) {
            let blocks = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    blocks: Arc::clone(&blocks),
                    fail_with: Arc::new(Mutex::new(None)),
                },
                blocks,
            )
        }

        fn with_failure(fail_with: TranscriptConsumerError) -> Self {
            let blocks = Arc::new(Mutex::new(Vec::new()));
            Self {
                blocks: Arc::clone(&blocks),
                fail_with: Arc::new(Mutex::new(Some(fail_with))),
            }
        }
    }

    impl TranscriptBlockConsumer for RecordingConsumer {
        fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError> {
            if let Some(err) = self.fail_with.lock().expect("lock").clone() {
                return Err(err);
            }
            self.blocks.lock().expect("lock").push(block);
            Ok(())
        }
    }

    fn assert_uuid_v4(block_id: &str) {
        let parsed = uuid::Uuid::parse_str(block_id).expect("block_id must be a valid UUID");
        assert_eq!(
            parsed.get_version(),
            Some(uuid::Version::Random),
            "block_id must be UUID v4"
        );
    }

    #[test]
    fn ignores_empty_and_whitespace_only_segments_without_advancing_sequence() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        for text in ["", "   ", "\t\n"] {
            emitter
                .on_segment(text, 500, "ja")
                .expect("empty segment should succeed");
        }

        assert_eq!(emitter.next_sequence(), 0);
        assert!(blocks.lock().expect("lock").is_empty());
    }

    #[test]
    fn sequence_increments_monotonically_for_non_empty_segments() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        emitter
            .on_segment("first", 1_000, "ja")
            .expect("first segment");
        emitter.on_segment("   ", 2_000, "ja").expect("ignored");
        emitter
            .on_segment("second", 3_000, "en")
            .expect("second segment");

        let blocks = blocks.lock().expect("lock");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].sequence, 1);
        assert_eq!(blocks[1].sequence, 2);
        assert_eq!(emitter.next_sequence(), 2);
    }

    #[test]
    fn start_timestamp_ms_matches_segment_input() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        emitter
            .on_segment("hello", 12_345, "ja")
            .expect("segment should emit");

        let block = &blocks.lock().expect("lock")[0];
        assert_eq!(block.start_timestamp_ms, 12_345);
    }

    #[test]
    fn start_timestamp_ms_reflects_batch_window_base_plus_segment_offset() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        // 160_000 samples @ 16 kHz = 10_000 ms batch window front + 250 ms segment offset.
        let window_base_ms = 10_000u64;
        let segment_offset_ms = 250u64;
        emitter
            .on_segment("batch aligned", window_base_ms + segment_offset_ms, "ja")
            .expect("segment should emit");

        let block = &blocks.lock().expect("lock")[0];
        assert_eq!(block.start_timestamp_ms, 10_250);
        assert_eq!(block.sequence, 1);
    }

    #[test]
    fn block_id_is_uuid_v4() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        emitter
            .on_segment("hello", 0, "und")
            .expect("segment should emit");

        assert_uuid_v4(&blocks.lock().expect("lock")[0].block_id);
    }

    #[test]
    fn trims_text_before_emitting() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        emitter
            .on_segment("  hello world  ", 0, "ja")
            .expect("segment should emit");

        assert_eq!(blocks.lock().expect("lock")[0].text, "hello world");
    }

    #[test]
    fn consumer_closed_maps_to_internal_transcribe_error() {
        let consumer = RecordingConsumer::with_failure(TranscriptConsumerError::Closed);
        let emitter = BlockEmitter::new(Arc::new(consumer));

        let err = emitter
            .on_segment("hello", 0, "ja")
            .expect_err("consumer closed should fail");

        assert_eq!(
            err,
            TranscribeError::Internal {
                detail: "transcript block consumer closed".to_string(),
            }
        );
    }

    #[test]
    fn consumer_internal_error_maps_to_internal_transcribe_error() {
        let consumer =
            RecordingConsumer::with_failure(TranscriptConsumerError::Internal("boom".to_string()));
        let emitter = BlockEmitter::new(Arc::new(consumer));

        let err = emitter
            .on_segment("hello", 0, "ja")
            .expect_err("consumer failure should propagate");

        assert_eq!(
            err,
            TranscribeError::Internal {
                detail: "boom".to_string(),
            }
        );
    }

    #[test]
    fn batch_cycle_segments_emit_monotonic_sequences_without_gaps() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter = BlockEmitter::new(Arc::new(consumer));

        // Simulates three batch cycles routed through run_inference_window → on_segment.
        emitter.on_segment("one", 0, "ja").expect("first batch");
        emitter
            .on_segment("   ", 100, "ja")
            .expect("empty segment skipped");
        emitter
            .on_segment("two", 30_000, "ja")
            .expect("second batch");
        emitter
            .on_segment("three", 60_000, "ja")
            .expect("third batch");

        let blocks = blocks.lock().expect("lock");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].sequence, 1);
        assert_eq!(blocks[1].sequence, 2);
        assert_eq!(blocks[2].sequence, 3);
        assert_eq!(emitter.next_sequence(), 3);
    }

    #[test]
    fn segment_sink_trait_is_object_safe_and_injectable() {
        let (consumer, blocks) = RecordingConsumer::new();
        let emitter: Arc<dyn TranscriptSegmentSink> =
            Arc::new(BlockEmitter::new(Arc::new(consumer)));

        emitter
            .on_segment("via trait", 99, "ja")
            .expect("trait dispatch should succeed");
        assert_eq!(blocks.lock().expect("lock")[0].text, "via trait");
    }
}
