use std::sync::Mutex;

use super::deps::{
    Arc, AtomicBool, AtomicU64, Duration, Instant, JoinHandle, Ordering, TranscribeError,
    TranscriptSegmentSink, thread,
};

use super::batch_cycle::{run_batch_cycle, samples_to_seconds};
use super::batch_window::{first_cycle_ready, next_cycle_ready, take_batch_window};
use super::engine::{ModelPathLoadable, SegmentEngine};
use super::pcm_buffer::{
    PcmBufferState, PcmDrainCallbacks, drain_pcm_loop, pcm_buffer_has_remaining,
    pcm_buffer_sample_count, samples_to_backlog_seconds,
};
use super::types::{
    BatchCycleCompletedCallback, BatchCycleStartedCallback, InferenceWindowLevelCallback,
};

#[derive(Clone)]
pub(crate) struct WorkerHooks {
    pub(crate) on_inference_latency_ms: Option<Arc<dyn Fn(u64) + Send + Sync>>,
    pub(crate) on_fatal: Option<Arc<dyn Fn(TranscribeError) + Send + Sync>>,
    pub(crate) on_engine_ready: Option<Arc<dyn Fn() + Send + Sync>>,
    pub(crate) on_inference_attempted: Option<Arc<dyn Fn() + Send + Sync>>,
    pub(crate) on_batch_cycle_started: Option<BatchCycleStartedCallback>,
    pub(crate) on_batch_cycle_completed: Option<BatchCycleCompletedCallback>,
    pub(crate) on_inference_window_level: Option<InferenceWindowLevelCallback>,
    pub(crate) rtrb_overflow_count: Option<Arc<AtomicU64>>,
    pub(crate) on_retention_limit: Option<Arc<dyn Fn() + Send + Sync>>,
    pub(crate) pcm_retention_limit_samples: Option<usize>,
    pub(crate) on_pcm_backlog_seconds: Option<Arc<dyn Fn(f64) + Send + Sync>>,
}

impl WorkerHooks {
    pub(crate) fn empty() -> Self {
        Self {
            on_inference_latency_ms: None,
            on_fatal: None,
            on_engine_ready: None,
            on_inference_attempted: None,
            on_batch_cycle_started: None,
            on_batch_cycle_completed: None,
            on_inference_window_level: None,
            rtrb_overflow_count: None,
            on_retention_limit: None,
            pcm_retention_limit_samples: None,
            on_pcm_backlog_seconds: None,
        }
    }
}

pub(crate) struct WorkerParams<E> {
    pub(crate) consumer: rtrb::Consumer<f32>,
    pub(crate) engine: E,
    pub(crate) model_path: Option<std::path::PathBuf>,
    pub(crate) sink: Arc<dyn TranscriptSegmentSink>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) pcm_draining: Arc<AtomicBool>,
    pub(crate) hooks: WorkerHooks,
    pub(crate) return_pcm_consumer: std::sync::mpsc::SyncSender<rtrb::Consumer<f32>>,
}

struct WorkerLoopState {
    transcribing_start: Instant,
    last_cycle_complete: Option<Instant>,
    backlog_after_last_cycle: bool,
    cycle_id: u64,
}

pub(crate) struct WorkerRuntime<'a, E> {
    pub(crate) running: &'a Arc<AtomicBool>,
    pub(crate) pcm_buffer: &'a Arc<Mutex<PcmBufferState>>,
    pub(crate) engine: &'a mut E,
    pub(crate) sink: &'a Arc<dyn TranscriptSegmentSink>,
    pub(crate) hooks: &'a WorkerHooks,
}

fn read_rtrb_overflow_count(counter: &Option<Arc<AtomicU64>>) -> u64 {
    counter
        .as_ref()
        .map(|value| value.load(Ordering::Relaxed))
        .unwrap_or(0)
}

fn report_pcm_backlog_if_changed(
    pcm_buffer: &Arc<Mutex<PcmBufferState>>,
    callback: &Option<Arc<dyn Fn(f64) + Send + Sync>>,
) {
    let Some(report) = callback else {
        return;
    };
    let seconds = samples_to_backlog_seconds(pcm_buffer_sample_count(pcm_buffer));
    let rounded = (seconds * 10.0).round() / 10.0;
    report(rounded);
}

fn spawn_pcm_drain_thread(
    consumer: rtrb::Consumer<f32>,
    pcm_buffer: Arc<Mutex<PcmBufferState>>,
    pcm_draining: Arc<AtomicBool>,
    callbacks: PcmDrainCallbacks,
) -> Option<JoinHandle<rtrb::Consumer<f32>>> {
    thread::Builder::new()
        .name("transcribe-pcm-drain".into())
        .spawn(move || drain_pcm_loop(consumer, pcm_buffer, pcm_draining, callbacks))
        .map_err(|err| eprintln!("failed to spawn pcm drain thread: {err}"))
        .ok()
}

fn join_drain_thread(
    drain_handle: Option<JoinHandle<rtrb::Consumer<f32>>>,
) -> Option<rtrb::Consumer<f32>> {
    drain_handle.and_then(|handle| handle.join().ok())
}

fn return_pcm_consumer_to_worker(
    return_tx: &std::sync::mpsc::SyncSender<rtrb::Consumer<f32>>,
    consumer: rtrb::Consumer<f32>,
) {
    if return_tx.send(consumer).is_err() {
        eprintln!("WARN: failed to return pcm consumer after transcribe worker shutdown");
    }
}

fn load_worker_engine<E: ModelPathLoadable>(
    engine: &mut E,
    model_path: &Option<std::path::PathBuf>,
) -> Result<(), TranscribeError> {
    if let Some(path) = model_path {
        engine.load_from_path_if_needed(path)
    } else if engine.is_loaded() {
        Ok(())
    } else {
        Err(TranscribeError::Internal {
            detail: "transcribe worker started without model path and engine is not loaded"
                .to_string(),
        })
    }
}

struct LoadFailureShutdown {
    running: Arc<AtomicBool>,
    pcm_draining: Arc<AtomicBool>,
    drain_handle: Option<JoinHandle<rtrb::Consumer<f32>>>,
    return_pcm_consumer: std::sync::mpsc::SyncSender<rtrb::Consumer<f32>>,
    on_fatal: Option<Arc<dyn Fn(TranscribeError) + Send + Sync>>,
}

fn stop_worker_after_load_failure(shutdown: LoadFailureShutdown, err: TranscribeError) {
    shutdown.running.store(false, Ordering::SeqCst);
    shutdown.pcm_draining.store(false, Ordering::SeqCst);
    if let Some(consumer) = join_drain_thread(shutdown.drain_handle) {
        return_pcm_consumer_to_worker(&shutdown.return_pcm_consumer, consumer);
    }
    if let Some(notify) = shutdown.on_fatal {
        thread::spawn(move || notify(err));
    }
}

fn process_batch_cycle_if_ready<E: ModelPathLoadable + SegmentEngine>(
    loop_state: &mut WorkerLoopState,
    runtime: &mut WorkerRuntime<'_, E>,
) {
    let unprocessed = pcm_buffer_sample_count(runtime.pcm_buffer);
    report_pcm_backlog_if_changed(runtime.pcm_buffer, &runtime.hooks.on_pcm_backlog_seconds);
    let ready = match loop_state.last_cycle_complete {
        None => first_cycle_ready(loop_state.transcribing_start, unprocessed),
        Some(completed_at) => next_cycle_ready(
            completed_at,
            unprocessed,
            loop_state.backlog_after_last_cycle,
        ),
    };

    if !ready {
        thread::sleep(Duration::from_millis(5));
        return;
    }

    let Some((pcm, base_samples)) = take_batch_window(runtime.pcm_buffer) else {
        return;
    };

    loop_state.cycle_id = loop_state.cycle_id.saturating_add(1);
    run_batch_cycle(
        loop_state.cycle_id,
        pcm,
        base_samples,
        samples_to_seconds(unprocessed),
        read_rtrb_overflow_count(&runtime.hooks.rtrb_overflow_count),
        runtime.engine.is_loaded(),
        runtime.engine,
        runtime.sink,
        runtime.hooks.on_inference_latency_ms.as_ref(),
        runtime.hooks.on_inference_attempted.as_ref(),
        runtime.hooks.on_batch_cycle_started.as_ref(),
        runtime.hooks.on_batch_cycle_completed.as_ref(),
        runtime.hooks.on_inference_window_level.as_ref(),
    );
    loop_state.last_cycle_complete = Some(Instant::now());
    loop_state.backlog_after_last_cycle = pcm_buffer_has_remaining(runtime.pcm_buffer);
}

fn run_transcribing_loop<E: ModelPathLoadable + SegmentEngine>(
    runtime: &mut WorkerRuntime<'_, E>,
) -> u64 {
    let mut loop_state = WorkerLoopState {
        transcribing_start: Instant::now(),
        last_cycle_complete: None,
        backlog_after_last_cycle: false,
        cycle_id: 0,
    };

    while runtime.running.load(Ordering::SeqCst) {
        process_batch_cycle_if_ready(&mut loop_state, runtime);
    }

    loop_state.cycle_id
}

fn flush_remaining_batch_cycles<E: ModelPathLoadable + SegmentEngine>(
    cycle_id: &mut u64,
    runtime: &mut WorkerRuntime<'_, E>,
) {
    while let Some((pcm, base_samples)) = take_batch_window(runtime.pcm_buffer) {
        *cycle_id = cycle_id.saturating_add(1);
        let flush_backlog_samples = pcm_buffer_sample_count(runtime.pcm_buffer);
        run_batch_cycle(
            *cycle_id,
            pcm,
            base_samples,
            samples_to_seconds(flush_backlog_samples),
            read_rtrb_overflow_count(&runtime.hooks.rtrb_overflow_count),
            runtime.engine.is_loaded(),
            runtime.engine,
            runtime.sink,
            runtime.hooks.on_inference_latency_ms.as_ref(),
            runtime.hooks.on_inference_attempted.as_ref(),
            runtime.hooks.on_batch_cycle_started.as_ref(),
            runtime.hooks.on_batch_cycle_completed.as_ref(),
            runtime.hooks.on_inference_window_level.as_ref(),
        );
    }
}

pub(crate) fn worker_loop<E: ModelPathLoadable>(params: WorkerParams<E>) {
    let WorkerParams {
        consumer,
        mut engine,
        model_path,
        sink,
        running,
        pcm_draining,
        hooks,
        return_pcm_consumer,
    } = params;

    let mut pcm_buffer_state = PcmBufferState::new();
    pcm_buffer_state.retention_limit_samples = hooks.pcm_retention_limit_samples;
    let pcm_buffer = Arc::new(Mutex::new(pcm_buffer_state));
    let drain_handle = spawn_pcm_drain_thread(
        consumer,
        Arc::clone(&pcm_buffer),
        Arc::clone(&pcm_draining),
        PcmDrainCallbacks {
            on_retention_limit: hooks.on_retention_limit.clone(),
            on_pcm_backlog_seconds: hooks.on_pcm_backlog_seconds.clone(),
        },
    );

    if let Err(err) = load_worker_engine(&mut engine, &model_path) {
        stop_worker_after_load_failure(
            LoadFailureShutdown {
                running,
                pcm_draining,
                drain_handle,
                return_pcm_consumer,
                on_fatal: hooks.on_fatal,
            },
            err,
        );
        return;
    }

    if !running.load(Ordering::SeqCst) {
        pcm_draining.store(false, Ordering::SeqCst);
        if let Some(consumer) = join_drain_thread(drain_handle) {
            return_pcm_consumer_to_worker(&return_pcm_consumer, consumer);
        }
        return;
    }

    if let Some(ref notify) = hooks.on_engine_ready {
        notify();
    }

    let mut runtime = WorkerRuntime {
        running: &running,
        pcm_buffer: &pcm_buffer,
        engine: &mut engine,
        sink: &sink,
        hooks: &hooks,
    };
    let mut cycle_id = run_transcribing_loop(&mut runtime);

    pcm_draining.store(false, Ordering::SeqCst);
    if let Some(consumer) = join_drain_thread(drain_handle) {
        return_pcm_consumer_to_worker(&return_pcm_consumer, consumer);
    }

    flush_remaining_batch_cycles(&mut cycle_id, &mut runtime);

    drop(engine);
}
