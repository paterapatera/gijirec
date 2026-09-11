//! Shared consumer-trait contract helpers for domain unit tests.

use std::sync::{Arc, Mutex};

#[cfg(test)]
pub(crate) fn assert_consumer_records_sequence(
    last_sequence: &Arc<Mutex<Option<u64>>>,
    expected: u64,
) {
    assert_eq!(*last_sequence.lock().expect("lock"), Some(expected));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::fixtures::sample_pcm_chunk;
    use crate::audio::pcm_chunk::{PcmChunkConsumer, PcmConsumerError};
    use crate::transcribe::transcript_block::{TranscriptBlockConsumer, TranscriptConsumerError};

    struct SequenceRecordingConsumer {
        last_sequence: Arc<Mutex<Option<u64>>>,
    }

    impl PcmChunkConsumer for SequenceRecordingConsumer {
        fn on_pcm_chunk(
            &self,
            chunk: crate::audio::pcm_chunk::PcmChunk,
        ) -> Result<(), PcmConsumerError> {
            *self.last_sequence.lock().expect("lock") = Some(chunk.sequence());
            Ok(())
        }
    }

    impl TranscriptBlockConsumer for SequenceRecordingConsumer {
        fn on_block_appended(
            &self,
            block: crate::transcribe::transcript_block::TranscriptBlock,
        ) -> Result<(), TranscriptConsumerError> {
            *self.last_sequence.lock().expect("lock") = Some(block.sequence);
            Ok(())
        }
    }

    fn recording_consumer() -> (SequenceRecordingConsumer, Arc<Mutex<Option<u64>>>) {
        let last_sequence = Arc::new(Mutex::new(None));
        let consumer = SequenceRecordingConsumer {
            last_sequence: Arc::clone(&last_sequence),
        };
        (consumer, last_sequence)
    }

    #[test]
    fn pcm_chunk_consumer_records_sequence() {
        let (consumer, last_sequence) = recording_consumer();
        consumer
            .on_pcm_chunk(sample_pcm_chunk(7))
            .expect("consumer accepts chunk");
        assert_consumer_records_sequence(&last_sequence, 7);
    }

    fn sample_block(sequence: u64) -> crate::transcribe::transcript_block::TranscriptBlock {
        crate::transcribe::transcript_block::TranscriptBlock::new(
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
            sequence,
            "hello".to_string(),
            sequence * 1_000,
            "ja".to_string(),
        )
        .expect("valid block")
    }

    #[test]
    fn transcript_block_consumer_records_sequence() {
        let (consumer, last_sequence) = recording_consumer();
        consumer
            .on_block_appended(sample_block(7))
            .expect("consumer accepts block");
        assert_consumer_records_sequence(&last_sequence, 7);
    }
}
