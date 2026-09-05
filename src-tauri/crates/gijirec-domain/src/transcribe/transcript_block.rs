//! Transcript block value object and downstream consumer trait.

use std::fmt;

/// Errors when constructing or validating a [`TranscriptBlock`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptBlockError {
    EmptyText,
    InvalidBlockId { detail: String },
}

impl fmt::Display for TranscriptBlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText => write!(f, "transcript block text must not be empty"),
            Self::InvalidBlockId { detail } => write!(f, "invalid block_id: {detail}"),
        }
    }
}

impl std::error::Error for TranscriptBlockError {}

/// Immutable transcript block per `docs/contracts/whisper-transcribe-blocks.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptBlock {
    pub block_id: String,
    pub sequence: u64,
    pub text: String,
    pub start_timestamp_ms: u64,
    pub language: String,
}

impl TranscriptBlock {
    /// Builds a contract-valid transcript block.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_id: String,
        sequence: u64,
        text: String,
        start_timestamp_ms: u64,
        language: String,
    ) -> Result<Self, TranscriptBlockError> {
        if text.is_empty() {
            return Err(TranscriptBlockError::EmptyText);
        }
        if !is_uuid_v4(&block_id) {
            return Err(TranscriptBlockError::InvalidBlockId {
                detail: "expected UUID v4".to_string(),
            });
        }

        Ok(Self {
            block_id,
            sequence,
            text,
            start_timestamp_ms,
            language,
        })
    }
}

fn is_uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    bytes[14] == b'4' && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
}

/// Errors returned by downstream transcript block consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptConsumerError {
    Closed,
    Internal(String),
}

impl fmt::Display for TranscriptConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => write!(f, "consumer queue closed"),
            Self::Internal(message) => write!(f, "consumer internal error: {message}"),
        }
    }
}

impl std::error::Error for TranscriptConsumerError {}

/// Downstream registration point for appended transcript blocks.
pub trait TranscriptBlockConsumer: Send + Sync {
    fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const BLOCK_ID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn sample_block(sequence: u64) -> TranscriptBlock {
        TranscriptBlock::new(
            BLOCK_ID.to_string(),
            sequence,
            "hello".to_string(),
            sequence * 1_000,
            "ja".to_string(),
        )
        .expect("valid block")
    }

    #[test]
    fn builds_contract_valid_block_with_all_fields() {
        let block = sample_block(1);

        assert_eq!(block.block_id, BLOCK_ID);
        assert_eq!(block.sequence, 1);
        assert_eq!(block.text, "hello");
        assert_eq!(block.start_timestamp_ms, 1_000);
        assert_eq!(block.language, "ja");
    }

    #[test]
    fn rejects_empty_text_block() {
        let err =
            TranscriptBlock::new(BLOCK_ID.to_string(), 1, String::new(), 0, "und".to_string())
                .unwrap_err();

        assert_eq!(err, TranscriptBlockError::EmptyText);
    }

    #[test]
    fn rejects_non_uuid_v4_block_id() {
        let err = TranscriptBlock::new(
            "not-a-uuid".to_string(),
            1,
            "hello".to_string(),
            0,
            "ja".to_string(),
        )
        .unwrap_err();

        assert_eq!(
            err,
            TranscriptBlockError::InvalidBlockId {
                detail: "expected UUID v4".to_string(),
            }
        );
    }

    #[test]
    fn consumer_trait_accepts_block() {
        struct MockConsumer {
            last_sequence: Arc<Mutex<Option<u64>>>,
        }

        impl TranscriptBlockConsumer for MockConsumer {
            fn on_block_appended(
                &self,
                block: TranscriptBlock,
            ) -> Result<(), TranscriptConsumerError> {
                *self.last_sequence.lock().expect("lock") = Some(block.sequence);
                Ok(())
            }
        }

        let last_sequence = Arc::new(Mutex::new(None));
        let consumer = MockConsumer {
            last_sequence: Arc::clone(&last_sequence),
        };
        let block = sample_block(7);
        consumer
            .on_block_appended(block)
            .expect("consumer accepts block");
        assert_eq!(*last_sequence.lock().expect("lock"), Some(7));
    }
}
