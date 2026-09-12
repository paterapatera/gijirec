use std::sync::Arc;
use std::time::Instant;

use gijirec_domain::transcribe::TranscriptSegmentSink;

use super::engine::{ModelPathLoadable, SegmentEngine};
use super::types::{
    BatchCycleCompleted, BatchCycleCompletedCallback, BatchCycleStarted, BatchCycleStartedCallback,
    InferenceWindowLevel, InferenceWindowLevelCallback,
};

pub(crate) const SAMPLE_RATE_HZ: u64 = 16_000;

/// Skip whisper.cpp when the window RMS is below this (near-silence).
pub(crate) const SILENCE_RMS_THRESHOLD: f32 = 0.008;

pub(crate) struct InferenceContext<'a, E> {
    pub(crate) engine: &'a mut E,
    pub(crate) sink: &'a Arc<dyn TranscriptSegmentSink>,
    pub(crate) on_latency: Option<&'a Arc<dyn Fn(u64) + Send + Sync>>,
    pub(crate) on_attempted: Option<&'a Arc<dyn Fn() + Send + Sync>>,
    pub(crate) on_window_level: Option<&'a InferenceWindowLevelCallback>,
}

pub(crate) struct InferenceOutcome {
    pub(crate) segments_count: usize,
}

pub(crate) fn window_rms(pcm: &[f32]) -> f32 {
    // Samples are post-`PcmIngestConsumer` gain (rtrb holds f32 after TRANSCRIBE_INGEST_GAIN).
    if pcm.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = pcm.iter().map(|sample| sample * sample).sum();
    (sum_sq / pcm.len() as f32).sqrt()
}

#[allow(clippy::too_many_arguments)] // batch cycle wires engine, sink, and observability callbacks.
pub(crate) fn run_batch_cycle<E: SegmentEngine + ModelPathLoadable>(
    cycle_id: u64,
    pcm: Vec<f32>,
    base_samples: u64,
    pcm_backlog_seconds: f64,
    rtrb_overflow_count: u64,
    engine_loaded: bool,
    engine: &mut E,
    sink: &Arc<dyn TranscriptSegmentSink>,
    on_latency: Option<&Arc<dyn Fn(u64) + Send + Sync>>,
    on_inference_attempted: Option<&Arc<dyn Fn() + Send + Sync>>,
    on_batch_cycle_started: Option<&BatchCycleStartedCallback>,
    on_batch_cycle_completed: Option<&BatchCycleCompletedCallback>,
    on_inference_window_level: Option<&InferenceWindowLevelCallback>,
) {
    let samples_count = pcm.len();
    let reload_path = if let Some(record) = on_batch_cycle_started {
        record(BatchCycleStarted {
            cycle_id,
            samples_count,
            pcm_backlog_seconds,
            rtrb_overflow_count,
        })
    } else {
        None
    };

    if let Some(path) = reload_path
        && let Err(err) = engine.reload_from_path(&path)
    {
        eprintln!("WARN: batch cycle model reload failed, continuing with prior model: {err}");
    }

    let cycle_start = Instant::now();
    let segments_count = if engine_loaded {
        let ctx = InferenceContext {
            engine,
            sink,
            on_latency,
            on_attempted: on_inference_attempted,
            on_window_level: on_inference_window_level,
        };
        run_inference_window(&pcm, base_samples, ctx).segments_count
    } else {
        0
    };

    if let Some(record) = on_batch_cycle_completed {
        record(BatchCycleCompleted {
            cycle_id,
            duration_ms: cycle_start.elapsed().as_millis() as u64,
            samples_count,
            segments_count,
        });
    }
}

pub(crate) fn run_inference_window<E: SegmentEngine>(
    pcm: &[f32],
    samples_before_buffer: u64,
    ctx: InferenceContext<'_, E>,
) -> InferenceOutcome {
    if pcm.is_empty() {
        return InferenceOutcome { segments_count: 0 };
    }

    let window_rms = window_rms(pcm);
    let inference_skipped = window_rms < SILENCE_RMS_THRESHOLD;
    if let Some(record) = ctx.on_window_level {
        record(InferenceWindowLevel {
            window_rms,
            samples_count: pcm.len(),
            inference_skipped,
        });
    }

    if inference_skipped {
        return InferenceOutcome { segments_count: 0 };
    }

    if let Some(attempted) = ctx.on_attempted {
        attempted();
    }

    let inference_start = Instant::now();
    let base_ms = samples_to_ms(samples_before_buffer);

    match ctx.engine.transcribe_pcm(pcm) {
        Ok(segments) => {
            let mut segments_count = 0usize;
            for segment in segments {
                let trimmed = segment.text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                segments_count += 1;
                let start_ms = base_ms.saturating_add(segment.start_ms.max(0) as u64);
                let _ = ctx.sink.on_segment(trimmed, start_ms, "auto");
            }
            if let Some(record) = ctx.on_latency {
                record(inference_start.elapsed().as_millis() as u64);
            }
            InferenceOutcome { segments_count }
        }
        Err(err) => {
            eprintln!("WARN: batch inference failed, continuing next cycle: {err}");
            if let Some(record) = ctx.on_latency {
                record(inference_start.elapsed().as_millis() as u64);
            }
            InferenceOutcome { segments_count: 0 }
        }
    }
}

pub(crate) fn samples_to_seconds(samples: usize) -> f64 {
    samples as f64 / SAMPLE_RATE_HZ as f64
}

pub(crate) fn samples_to_ms(samples: u64) -> u64 {
    samples.saturating_mul(1_000) / SAMPLE_RATE_HZ
}
