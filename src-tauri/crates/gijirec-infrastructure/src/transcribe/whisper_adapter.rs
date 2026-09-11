//! whisper.cpp wrapper for local STT inference (ADR-0003).

use std::path::Path;
use std::sync::Arc;

use gijirec_domain::transcribe::TranscribeError;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Inference segment returned by whisper.cpp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperSegment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

struct LoadedWhisper {
    context: WhisperContext,
    state: whisper_rs::WhisperState,
}

/// Local whisper.cpp adapter backed by `whisper-rs`.
pub struct WhisperCppAdapter {
    model: Option<LoadedWhisper>,
    on_progress: Option<Arc<dyn Fn(i32) + Send + Sync>>,
}

impl Default for WhisperCppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WhisperCppAdapter {
    pub fn new() -> Self {
        Self {
            model: None,
            on_progress: None,
        }
    }

    pub fn set_progress_hook(&mut self, hook: Arc<dyn Fn(i32) + Send + Sync>) {
        self.on_progress = Some(hook);
    }

    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    /// Vocabulary size of the loaded model; zero when unloaded.
    pub fn context_vocab_size(&self) -> i32 {
        self.model
            .as_ref()
            .map(|model| model.context.n_vocab())
            .unwrap_or(0)
    }

    /// Audio context length of the loaded model; zero when unloaded.
    pub fn context_audio_ctx(&self) -> i32 {
        self.model
            .as_ref()
            .map(|model| model.context.n_audio_ctx())
            .unwrap_or(0)
    }

    fn cpu_context_params() -> WhisperContextParameters<'static> {
        let mut params = WhisperContextParameters::default();
        #[cfg(not(target_os = "macos"))]
        {
            params.use_gpu = false;
            params.flash_attn = false;
        }
        params
    }

    /// Loads a whisper model from `path`. Failures map to [`TranscribeError::ModelCorrupt`].
    pub fn load_model(&mut self, path: &Path) -> Result<(), TranscribeError> {
        let path_display = path.display().to_string();
        let context = WhisperContext::new_with_params(
            path.to_str().ok_or_else(|| TranscribeError::ModelCorrupt {
                detail: format!("invalid model path: {path_display}"),
            })?,
            Self::cpu_context_params(),
        )
        .map_err(|err| TranscribeError::ModelCorrupt {
            detail: format!("failed to load whisper model at {path_display}: {err}"),
        })?;

        let state = context
            .create_state()
            .map_err(|err| TranscribeError::InferenceFailed {
                detail: format!("failed to initialize whisper state: {err}"),
            })?;

        self.model = Some(LoadedWhisper { context, state });
        Ok(())
    }

    /// Drops any loaded model and loads from `path` (batch-cycle variant switch).
    pub fn reload_model(&mut self, path: &Path) -> Result<(), TranscribeError> {
        self.model = None;
        self.load_model(path)
    }

    /// Runs inference over 16 kHz mono f32 PCM, reusing a single whisper state.
    pub fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }

        let model = self
            .model
            .as_mut()
            .ok_or_else(|| TranscribeError::Internal {
                detail: "whisper context not loaded".to_string(),
            })?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("ja"));
        params.set_n_threads(inference_thread_count());
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_context(true);
        // Timestamp tokens give the decoder natural stopping points and split a long
        // window into segments. whisper.cpp treats `single_segment` and `no_timestamps`
        // alike, and either one invites repetition loops on Japanese speech.
        params.set_no_timestamps(false);
        params.set_single_segment(false);
        params.set_temperature_inc(0.0);
        params.set_suppress_blank(true);
        // Hard cap per segment: if the decoder still loops, bound the damage and latency.
        params.set_max_tokens(MAX_TOKENS_PER_SEGMENT);
        params.set_audio_ctx(audio_ctx_for_pcm(pcm.len()));
        if let Some(hook) = self.on_progress.clone() {
            params.set_progress_callback_safe(move |percent: i32| hook(percent));
        }

        model
            .state
            .full(params, pcm)
            .map_err(|err| TranscribeError::InferenceFailed {
                detail: err.to_string(),
            })?;

        let mut segments = Vec::new();
        for segment in model.state.as_iter() {
            let text = segment
                .to_str_lossy()
                .map_err(|err| TranscribeError::InferenceFailed {
                    detail: err.to_string(),
                })?;
            segments.push(WhisperSegment {
                text: text.into_owned(),
                start_ms: segment.start_timestamp().saturating_mul(10),
                end_ms: segment.end_timestamp().saturating_mul(10),
            });
        }

        Ok(segments)
    }
}

/// Upper bound for ggml threads. kotoba-whisper carries a large-v3 encoder; a short
/// window at `audio_ctx=512` needs ~8 threads to stay ahead of real time on CPU. The
/// remaining cores are left to capture, UI, and the OS.
const MAX_INFERENCE_THREADS: usize = 8;

/// Per-segment token cap. A 10 s Japanese utterance is roughly 40-60 tokens, so this
/// never truncates real speech but stops a repetition loop within one segment.
const MAX_TOKENS_PER_SEGMENT: i32 = 128;

fn inference_thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(2, MAX_INFERENCE_THREADS) as i32)
        .unwrap_or(2)
}

/// Lower bound for `audio_ctx`. whisper.cpp documents 512 as the smallest encoder
/// context that keeps acceptable quality; below that the model hallucinates short
/// nonsense words on Japanese speech.
const MIN_AUDIO_CTX_FRAMES: i32 = 512;

/// Encoder frames for the given PCM length. whisper.cpp defaults to 1500 frames (30 s)
/// even for a short streaming window, so streaming must shrink `audio_ctx`.
fn audio_ctx_for_pcm(sample_count: usize) -> i32 {
    let frames = (sample_count as u64).saturating_mul(50) / 16_000;
    i32::try_from(frames)
        .unwrap_or(1500)
        .clamp(MIN_AUDIO_CTX_FRAMES, 1500)
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

    fn loaded_whisper_adapter() -> WhisperCppAdapter {
        let model_path = std::env::var("GIJIREC_WHISPER_TEST_MODEL")
            .expect("set GIJIREC_WHISPER_TEST_MODEL to a valid ggml whisper model path");
        let mut adapter = WhisperCppAdapter::new();
        adapter
            .load_model(Path::new(&model_path))
            .expect("load local whisper model");
        adapter
    }

    #[test]
    fn load_model_failure_maps_to_model_corrupt() {
        let mut adapter = WhisperCppAdapter::new();
        let err = gijirec_domain::transcribe::missing_whisper_model_load_err(|path| {
            adapter.load_model(path)
        });
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
    fn transcribe_empty_pcm_returns_empty_segments_without_inference() {
        let mut adapter = WhisperCppAdapter::new();
        let segments = adapter
            .transcribe_pcm(&[])
            .expect("empty pcm should succeed without loaded context");
        assert!(segments.is_empty());
    }

    #[test]
    fn transcribe_without_loaded_context_returns_internal() {
        let mut adapter = WhisperCppAdapter::new();
        let err = adapter
            .transcribe_pcm(&[0.0_f32; 16_000])
            .expect_err("unloaded context should fail");
        assert!(matches!(err, TranscribeError::Internal { .. }));
    }

    #[test]
    fn full_params_keep_timestamps_and_cap_segment_tokens() {
        let source = include_str!("whisper_adapter.rs");
        let production = source
            .split("mod tests")
            .next()
            .expect("whisper_adapter.rs must define tests module");
        assert!(
            production.contains("set_no_timestamps(false)")
                && production.contains("set_single_segment(false)"),
            "streaming inference must keep timestamp tokens so segments split naturally"
        );
        assert!(
            production.contains("set_max_tokens(MAX_TOKENS_PER_SEGMENT)"),
            "streaming inference must cap tokens per segment to bound repetition loops"
        );
        assert!(
            !production.contains("set_no_timestamps(true)")
                && !production.contains("set_single_segment(true)"),
            "single-segment / no-timestamp decoding invites repetition loops"
        );
    }

    #[test]
    fn audio_ctx_matches_pcm_duration_not_full_30s_encoder() {
        // Short windows sit below the quality floor, so clamp up to 512.
        assert_eq!(audio_ctx_for_pcm(48_000), 512);
        assert_eq!(audio_ctx_for_pcm(16_000), 512);
        // 10 s max window: proportional (500 frames) still clamps to 512.
        assert_eq!(audio_ctx_for_pcm(160_000), 512);
        // 15 s of PCM: proportional (750 frames), still below the 30 s default.
        assert_eq!(audio_ctx_for_pcm(240_000), 750);
        assert_eq!(audio_ctx_for_pcm(480_000), 1500);
        assert_eq!(audio_ctx_for_pcm(960_000), 1500);
    }

    #[test]
    #[ignore = "requires GIJIREC_WHISPER_TEST_MODEL pointing to a valid ggml whisper model"]
    fn smoke_transcribe_one_second_silence() {
        let mut adapter = loaded_whisper_adapter();
        adapter
            .transcribe_pcm(&vec![0.0_f32; 16_000])
            .expect("transcribe one second of silence");
    }

    #[test]
    #[ignore = "requires GIJIREC_WHISPER_TEST_MODEL pointing to a valid ggml whisper model"]
    fn smoke_transcribe_five_second_silence() {
        let mut adapter = loaded_whisper_adapter();
        adapter
            .transcribe_pcm(&vec![0.0_f32; 80_000])
            .expect("transcribe five seconds of silence");
    }

    #[test]
    #[ignore = "requires GIJIREC_WHISPER_TEST_MODEL pointing to a valid ggml whisper model"]
    fn smoke_load_and_transcribe_synthetic_pcm() {
        let mut adapter = loaded_whisper_adapter();
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
