//! whisper-cpp-plus wrapper for local STT inference (ADR-0003).

use std::io::Cursor;
use std::path::Path;

use gijirec_domain::transcribe::TranscribeError;
use whisper_cpp_plus::{
    FullParams, PcmFormat, PcmReader, PcmReaderConfig, SamplingStrategy, Segment, WhisperContext,
    WhisperStreamPcm, WhisperStreamPcmConfig,
};

/// Inference segment returned by whisper-cpp-plus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperSegment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Local whisper.cpp adapter wrapping [`WhisperContext`] and [`WhisperStreamPcm`].
pub struct WhisperCppAdapter {
    context: Option<WhisperContext>,
}

impl Default for WhisperCppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WhisperCppAdapter {
    pub fn new() -> Self {
        Self { context: None }
    }

    pub fn is_loaded(&self) -> bool {
        self.context.is_some()
    }

    /// Vocabulary size of the loaded model; zero when unloaded.
    pub fn context_vocab_size(&self) -> i32 {
        self.context
            .as_ref()
            .map(WhisperContext::n_vocab)
            .unwrap_or(0)
    }

    /// Audio context length of the loaded model; zero when unloaded.
    pub fn context_audio_ctx(&self) -> i32 {
        self.context
            .as_ref()
            .map(WhisperContext::n_audio_ctx)
            .unwrap_or(0)
    }

    /// VAD-driven streaming config per design (`length_ms=5000`, `use_vad=true`).
    pub fn stream_pcm_config() -> WhisperStreamPcmConfig {
        WhisperStreamPcmConfig {
            length_ms: 5000,
            use_vad: true,
            ..Default::default()
        }
    }

    /// Loads a whisper model from `path`. Failures map to [`TranscribeError::ModelCorrupt`].
    pub fn load_model(&mut self, path: &Path) -> Result<(), TranscribeError> {
        let path_display = path.display().to_string();
        match WhisperContext::new(&path_display) {
            Ok(ctx) => {
                self.context = Some(ctx);
                Ok(())
            }
            Err(err) => Err(TranscribeError::ModelCorrupt {
                detail: format!("failed to load whisper model at {path_display}: {err}"),
            }),
        }
    }

    /// Runs VAD-driven streaming inference over 16 kHz mono f32 PCM.
    pub fn transcribe_pcm(&self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }

        let ctx = self
            .context
            .as_ref()
            .ok_or_else(|| TranscribeError::Internal {
                detail: "whisper context not loaded".to_string(),
            })?;

        let reader = pcm_reader_from_samples(pcm);
        run_stream_pcm(ctx, reader)
    }
}

fn map_segment(seg: &Segment) -> WhisperSegment {
    WhisperSegment {
        text: seg.text.clone(),
        start_ms: seg.start_ms,
        end_ms: seg.end_ms,
    }
}

fn pcm_reader_from_samples(samples: &[f32]) -> PcmReader {
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let config = PcmReaderConfig {
        sample_rate: 16_000,
        format: PcmFormat::F32,
        ..Default::default()
    };
    PcmReader::new(Box::new(Cursor::new(bytes)), config)
}

fn run_stream_pcm(
    ctx: &WhisperContext,
    reader: PcmReader,
) -> Result<Vec<WhisperSegment>, TranscribeError> {
    let params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let config = WhisperCppAdapter::stream_pcm_config();
    let mut stream = WhisperStreamPcm::new(ctx, params, config, reader).map_err(|err| {
        TranscribeError::InferenceFailed {
            detail: err.to_string(),
        }
    })?;

    // `WhisperStreamPcm::run` blocks until the reader hits EOF and drains the ring buffer;
    // no additional sleep is required before or during the loop.
    let mut segments = Vec::new();
    stream
        .run(|segs, _start_ms, _end_ms| {
            for seg in segs {
                segments.push(map_segment(seg));
            }
        })
        .map_err(|err| TranscribeError::InferenceFailed {
            detail: err.to_string(),
        })?;

    Ok(segments)
}

/// Generates synthetic PCM with an energy burst to exercise VAD-driven streaming.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn synthetic_pcm_with_activity(duration_secs: f32) -> Vec<f32> {
    let sample_rate = 16_000_i32;
    let total_samples = (duration_secs * sample_rate as f32) as usize;
    let mut pcm = vec![0.0_f32; total_samples];

    let burst_start = sample_rate as usize;
    let burst_end = (4.0 * sample_rate as f32) as usize;
    let effective_end = burst_end.min(total_samples);
    if effective_end > burst_start {
        for (offset, sample) in pcm[burst_start..effective_end].iter_mut().enumerate() {
            let idx = burst_start + offset;
            let t = idx as f32 / sample_rate as f32;
            *sample = 0.25 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
        }
    }

    pcm
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::transcribe::TranscribeErrorCode;
    use std::fs;

    #[test]
    fn load_model_failure_maps_to_model_corrupt() {
        let mut adapter = WhisperCppAdapter::new();
        let err = adapter
            .load_model(Path::new("/nonexistent/gijirec-model.bin"))
            .expect_err("missing model should fail");

        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        assert_eq!(err.to_user_facing().code, TranscribeErrorCode::ModelCorrupt);
        assert!(!adapter.is_loaded());
    }

    #[test]
    fn corrupt_file_maps_to_model_corrupt() {
        let dir =
            std::env::temp_dir().join(format!("gijirec-whisper-corrupt-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("corrupt.bin");
        fs::write(&path, b"not-a-whisper-model").expect("write corrupt file");

        let mut adapter = WhisperCppAdapter::new();
        let err = adapter
            .load_model(&path)
            .expect_err("corrupt model should fail");

        let _ = fs::remove_dir_all(&dir);
        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        assert_eq!(
            err.to_user_facing().code.as_str(),
            TranscribeErrorCode::ModelCorrupt.as_str()
        );
        assert!(!adapter.is_loaded());
    }

    #[test]
    fn stream_pcm_config_uses_vad_and_length_ms() {
        let config = WhisperCppAdapter::stream_pcm_config();
        assert!(config.use_vad);
        assert_eq!(config.length_ms, 5000);
    }

    #[test]
    fn transcribe_empty_pcm_returns_empty_segments_without_inference() {
        let adapter = WhisperCppAdapter::new();
        let segments = adapter
            .transcribe_pcm(&[])
            .expect("empty pcm should succeed without loaded context");
        assert!(segments.is_empty());
    }

    #[test]
    fn transcribe_without_loaded_context_returns_internal() {
        let adapter = WhisperCppAdapter::new();
        let err = adapter
            .transcribe_pcm(&[0.0_f32; 16_000])
            .expect_err("unloaded context should fail");
        assert!(matches!(err, TranscribeError::Internal { .. }));
    }

    #[test]
    #[ignore = "requires GIJIREC_WHISPER_TEST_MODEL pointing to a valid ggml whisper model"]
    fn smoke_load_and_transcribe_synthetic_pcm() {
        let model_path = std::env::var("GIJIREC_WHISPER_TEST_MODEL")
            .expect("set GIJIREC_WHISPER_TEST_MODEL to a valid ggml whisper model path");

        let mut adapter = WhisperCppAdapter::new();
        adapter
            .load_model(Path::new(&model_path))
            .expect("load local whisper model");
        assert!(adapter.is_loaded());

        let pcm = synthetic_pcm_with_activity(6.0);
        let segments = adapter
            .transcribe_pcm(&pcm)
            .expect("transcribe synthetic pcm");
        assert!(
            !segments.is_empty(),
            "expected at least one segment from synthetic speech"
        );
        assert!(
            segments
                .iter()
                .all(|segment| !segment.text.trim().is_empty()),
            "returned segments should contain non-empty text"
        );
    }

    #[test]
    fn smoke_loaded_context_exposes_model_metadata() {
        let model_path = match std::env::var("GIJIREC_WHISPER_TEST_MODEL") {
            Ok(path) if !path.is_empty() => path,
            _ => return,
        };

        let mut adapter = WhisperCppAdapter::new();
        adapter
            .load_model(Path::new(&model_path))
            .expect("load local whisper model");
        assert!(adapter.is_loaded());
        assert!(adapter.context_vocab_size() > 0);
        assert!(adapter.context_audio_ctx() > 0);
    }
}
