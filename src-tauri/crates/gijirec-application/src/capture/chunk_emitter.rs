//! 100 ms PcmChunk generation from mixed f32 samples.

use gijirec_domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};

/// Duration of one emitted chunk in milliseconds.
const CHUNK_DURATION_MS: u64 = 100;

/// Converts mixed f32 samples into contract-valid 100 ms [`PcmChunk`] values.
#[derive(Debug)]
pub struct ChunkEmitter {
    buffer: Vec<f32>,
    next_sequence: u64,
    stopped: bool,
}

impl ChunkEmitter {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            next_sequence: 0,
            stopped: false,
        }
    }

    /// Appends mixed mono f32 samples (post-mixer, nominal range ±1.0).
    pub fn push_mixed(&mut self, samples: &[f32]) {
        if self.stopped || samples.is_empty() {
            return;
        }
        self.buffer.extend_from_slice(samples);
    }

    /// Emits all complete 100 ms chunks currently buffered.
    pub fn emit_ready(&mut self) -> Vec<PcmChunk> {
        if self.stopped {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let frame_len = CHUNK_FRAME_COUNT as usize;

        while self.buffer.len() >= frame_len {
            let frame: Vec<f32> = self.buffer.drain(..frame_len).collect();
            let samples = f32_to_i16(&frame);
            let sequence = self.next_sequence;
            let timestamp_ms = sequence * CHUNK_DURATION_MS;
            let chunk = PcmChunk::new(sequence, samples, timestamp_ms)
                .expect("1600-sample chunk must satisfy contract");
            self.next_sequence += 1;
            chunks.push(chunk);
        }

        chunks
    }

    /// Discards partial samples and prevents further emission (stop contract).
    pub fn stop(&mut self) {
        self.stopped = true;
        self.buffer.clear();
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn pending_samples(&self) -> usize {
        self.buffer.len()
    }
}

impl Default for ChunkEmitter {
    fn default() -> Self {
        Self::new()
    }
}

fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|s| {
            let clamped = s.clamp(-1.0, 1.0);
            (clamped * 32_767.0).round() as i16
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::audio::SAMPLE_RATE_HZ;

    fn push_frames(emitter: &mut ChunkEmitter, frames: usize, value: f32) {
        let samples = vec![value; frames];
        emitter.push_mixed(&samples);
    }

    #[test]
    // Testing Strategy 3: 1600 サンプル境界で sequence 単調増加 (req 2.3)
    fn emits_1600_sample_chunks_with_monotonic_sequence() {
        let mut emitter = ChunkEmitter::new();
        push_frames(&mut emitter, CHUNK_FRAME_COUNT as usize * 3, 0.25);

        let chunks = emitter.emit_ready();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].sequence(), 0);
        assert_eq!(chunks[1].sequence(), 1);
        assert_eq!(chunks[2].sequence(), 2);
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(
                chunk.sequence(),
                i as u64,
                "sequence must increase monotonically"
            );
            assert_eq!(chunk.frame_count(), CHUNK_FRAME_COUNT);
            assert_eq!(chunk.samples().len(), CHUNK_FRAME_COUNT as usize);
            assert_eq!(chunk.sample_rate_hz(), SAMPLE_RATE_HZ);
        }
    }

    #[test]
    // Testing Strategy 3: 連続入力でも sequence にギャップがない
    fn sequence_has_no_gaps_on_continuous_input() {
        let mut emitter = ChunkEmitter::new();
        for expected_sequence in 0..5_u64 {
            push_frames(&mut emitter, CHUNK_FRAME_COUNT as usize, 0.1);
            let batch = emitter.emit_ready();
            assert_eq!(batch.len(), 1);
            let chunk = &batch[0];
            assert_eq!(chunk.sequence(), expected_sequence);
            assert_eq!(chunk.frame_count(), CHUNK_FRAME_COUNT);
            assert_eq!(chunk.samples().len(), CHUNK_FRAME_COUNT as usize);
        }
        assert_eq!(emitter.next_sequence(), 5);
    }

    #[test]
    fn stop_discards_partial_chunk_and_blocks_emission() {
        let mut emitter = ChunkEmitter::new();
        push_frames(&mut emitter, CHUNK_FRAME_COUNT as usize, 0.5);
        push_frames(&mut emitter, 800, 0.5);

        emitter.stop();
        assert_eq!(emitter.pending_samples(), 0);

        push_frames(&mut emitter, CHUNK_FRAME_COUNT as usize * 2, 0.5);
        let chunks = emitter.emit_ready();
        assert!(chunks.is_empty(), "no chunks after stop");
        assert!(emitter.is_stopped());
    }

    #[test]
    fn timestamp_ms_aligns_with_sequence() {
        let mut emitter = ChunkEmitter::new();
        push_frames(&mut emitter, CHUNK_FRAME_COUNT as usize, 0.0);
        let chunks = emitter.emit_ready();
        assert_eq!(chunks[0].timestamp_ms(), 0);
        assert_eq!(chunks[0].sequence(), 0);

        push_frames(&mut emitter, CHUNK_FRAME_COUNT as usize, 0.0);
        let more = emitter.emit_ready();
        assert_eq!(more[0].timestamp_ms(), 100);
        assert_eq!(more[0].sequence(), 1);
    }
}
