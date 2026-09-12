use std::sync::Arc;

/// Structured fields for a batch inference cycle start event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchCycleStarted {
    pub cycle_id: u64,
    pub samples_count: usize,
    pub pcm_backlog_seconds: f64,
    pub rtrb_overflow_count: u64,
}

/// Structured fields for a batch inference cycle completion event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchCycleCompleted {
    pub cycle_id: u64,
    pub duration_ms: u64,
    pub samples_count: usize,
    pub segments_count: usize,
}

/// PCM level metrics for one whisper.cpp inference window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InferenceWindowLevel {
    pub window_rms: f32,
    pub samples_count: usize,
    pub inference_skipped: bool,
}

pub(crate) type BatchCycleStartedCallback =
    Arc<dyn Fn(BatchCycleStarted) -> Option<std::path::PathBuf> + Send + Sync>;
pub(crate) type BatchCycleCompletedCallback = Arc<dyn Fn(BatchCycleCompleted) + Send + Sync>;
pub(crate) type InferenceWindowLevelCallback = Arc<dyn Fn(InferenceWindowLevel) + Send + Sync>;
