//! PCM chunk value object and downstream consumer trait.

use std::fmt;

/// Fixed sample rate for downstream whisper-transcribe compatibility.
pub const SAMPLE_RATE_HZ: u32 = 16_000;

/// Mono channel count.
pub const CHANNELS: u8 = 1;

/// 100 ms of audio at 16 kHz.
pub const CHUNK_FRAME_COUNT: u32 = 1_600;

/// Sample format for normalized PCM output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    Int16Le,
}

/// Errors when constructing or validating a [`PcmChunk`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcmChunkError {
    InvalidFrameCount { expected: u32, actual: usize },
}

impl fmt::Display for PcmChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrameCount { expected, actual } => {
                write!(f, "expected {expected} samples per chunk, got {actual}")
            }
        }
    }
}

impl std::error::Error for PcmChunkError {}

/// Immutable normalized PCM chunk per `docs/contracts/audio-capture-pcm.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmChunk {
    sequence: u64,
    sample_rate_hz: u32,
    channels: u8,
    sample_format: SampleFormat,
    samples: Vec<i16>,
    frame_count: u32,
    timestamp_ms: u64,
}

impl PcmChunk {
    /// Builds a contract-valid 100 ms chunk.
    pub fn new(sequence: u64, samples: Vec<i16>, timestamp_ms: u64) -> Result<Self, PcmChunkError> {
        let actual = samples.len();
        if actual != CHUNK_FRAME_COUNT as usize {
            return Err(PcmChunkError::InvalidFrameCount {
                expected: CHUNK_FRAME_COUNT,
                actual,
            });
        }

        Ok(Self {
            sequence,
            sample_rate_hz: SAMPLE_RATE_HZ,
            channels: CHANNELS,
            sample_format: SampleFormat::Int16Le,
            samples,
            frame_count: actual as u32,
            timestamp_ms,
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    pub fn channels(&self) -> u8 {
        self.channels
    }

    pub fn sample_format(&self) -> SampleFormat {
        self.sample_format
    }

    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    /// Byte size of PCM payload (`frame_count * 2` for Int16Le).
    pub fn byte_len(&self) -> usize {
        self.frame_count as usize * 2
    }
}

/// Errors returned by downstream PCM consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcmConsumerError {
    Disconnected,
    Internal(String),
}

impl fmt::Display for PcmConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "pcm consumer disconnected"),
            Self::Internal(message) => write!(f, "pcm consumer internal error: {message}"),
        }
    }
}

impl std::error::Error for PcmConsumerError {}

/// Downstream registration point for emitted PCM chunks.
pub trait PcmChunkConsumer: Send + Sync {
    fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_chunk(sequence: u64) -> PcmChunk {
        PcmChunk::new(
            sequence,
            vec![0_i16; CHUNK_FRAME_COUNT as usize],
            sequence * 100,
        )
        .expect("valid chunk")
    }

    #[test]
    fn builds_100ms_chunk_with_contract_fields() {
        let chunk = sample_chunk(1);

        assert_eq!(chunk.sequence(), 1);
        assert_eq!(chunk.sample_rate_hz(), 16_000);
        assert_eq!(chunk.channels(), 1);
        assert_eq!(chunk.sample_format(), SampleFormat::Int16Le);
        assert_eq!(chunk.frame_count(), 1_600);
        assert_eq!(chunk.samples().len(), 1_600);
        assert_eq!(chunk.byte_len(), 3_200);
        assert_eq!(chunk.timestamp_ms(), 100);
    }

    #[test]
    fn rejects_non_1600_sample_chunks() {
        let err = PcmChunk::new(0, vec![0_i16; 1_599], 0).unwrap_err();
        assert_eq!(
            err,
            PcmChunkError::InvalidFrameCount {
                expected: 1_600,
                actual: 1_599,
            }
        );
    }

    #[test]
    fn frame_count_matches_samples_len() {
        let chunk = sample_chunk(42);
        assert_eq!(chunk.frame_count() as usize, chunk.samples().len());
    }

    #[test]
    fn consumer_trait_accepts_chunk() {
        struct MockConsumer {
            last_sequence: Arc<Mutex<Option<u64>>>,
        }

        impl PcmChunkConsumer for MockConsumer {
            fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError> {
                *self.last_sequence.lock().expect("lock") = Some(chunk.sequence());
                Ok(())
            }
        }

        let last_sequence = Arc::new(Mutex::new(None));
        let consumer = MockConsumer {
            last_sequence: Arc::clone(&last_sequence),
        };
        let chunk = sample_chunk(7);
        consumer
            .on_pcm_chunk(chunk)
            .expect("consumer accepts chunk");
        assert_eq!(*last_sequence.lock().expect("lock"), Some(7));
    }
}
