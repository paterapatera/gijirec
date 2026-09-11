use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
};
use gijirec_presentation::application::capture_audio_controls::{
    CaptureAudioControlsService, CaptureAudioControlsStore, DefaultCaptureAudioControlsService,
};
use gijirec_presentation::application::device_selection::DeviceSelectionStore;
#[cfg(target_os = "macos")]
use gijirec_presentation::application::device_selection::MacosSpeakerPreflight;
use gijirec_presentation::application::device_selection::{
    DefaultDeviceSelectionService, DeviceSelectionService, NoopSpeakerPreflight, SystemClock,
};
use gijirec_presentation::application::transcribe::block_emitter::BlockEmitter;
use gijirec_presentation::application::transcribe::model_orchestrator::ApplyVariantOutcome;
use gijirec_presentation::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use gijirec_presentation::domain::transcribe::{
    TranscribeError, TranscriptBlockConsumer, TranscriptSegmentSink,
};
use gijirec_presentation::infrastructure::audio::device_enumerator::AudioDeviceEnumerator;
use gijirec_presentation::infrastructure::transcribe::TranscribeWorker;
use gijirec_presentation::transcribe::lifecycle_hook::DEFAULT_TRANSCRIBE_STOP_TIMEOUT;
use gijirec_presentation::transcribe::{
    IngestLevelEmitter, PcmIngestConsumer, StallClock, TranscribeEventEmitter,
    TranscribeLifecycleHook, TranscribeWorkerPortAdapter, TranscriptBlockBus,
    WhisperContextPortAdapter,
};

use crate::capture_ports::CaptureStreamHandles;
use crate::capture_processing::{CapturePipelineState, CaptureProcessingGate};
use crate::device_selection_observability::TracingDeviceSelectionObservability;

use super::ComposedCapture;
use super::audio_controls::{
    CachingIngestLevelEventEmitter, CaptureAudioControlsHookDeps,
    CaptureAudioControlsProcessingHook,
};
use super::late_bound::{
    CaptureAudioControlsEventsProxy, DeviceSelectionEventsProxy,
    LateBoundCaptureAudioControlsEvents, LateBoundDeviceSelectionEvents,
};
use super::model_stack::SharedModelOrchestrator;
use super::port_adapters::{
    CaptureAudioControlsApplyPortAdapter, CaptureSelectionPortAdapter, ComposeIngestSourcePort,
    DeviceEnumeratorPortAdapter, OrchestratorCapturePhasePort,
};

/// 16 kHz PCM sample rate for the transcribe pipeline.
const PCM_SAMPLE_RATE_HZ: u32 = 16_000;

/// One 30 s inference window at [`PCM_SAMPLE_RATE_HZ`].
pub(crate) const PCM_INFERENCE_WINDOW_SAMPLES: usize = 30 * PCM_SAMPLE_RATE_HZ as usize;

/// Number of 100 ms PCM chunks between ingest RMS summary logs (5 s).
const PCM_INGEST_RMS_LOG_INTERVAL_CHUNKS: u64 = 50;

/// rtrb capacity between [`PcmIngestConsumer`] and the transcribe PCM drain thread.
///
/// Worst-case headroom: slow 30 s window inference while capture continues at 16 kHz,
/// plus brief drain-thread stalls on the downstream `VecDeque` mutex during window
/// extraction. Must materially exceed [`PCM_INFERENCE_WINDOW_SAMPLES`] so ingest never
/// fails with `Internal("rtrb buffer full")` (Req 2.1/2.2). Long-term backlog retreat
/// lives in task 2.1; compose only sizes this burst buffer (10 windows ≈ 5 min @ 16 kHz).
pub(crate) const PCM_RTRB_CAPACITY_SAMPLES: usize = PCM_INFERENCE_WINDOW_SAMPLES * 10;

#[derive(Debug)]
pub(crate) struct PcmIngestRmsAccumulator {
    count: u64,
    min_rms: f32,
    max_rms: f32,
    sum_rms: f64,
}

impl PcmIngestRmsAccumulator {
    pub(crate) fn new() -> Self {
        Self {
            count: 0,
            min_rms: f32::INFINITY,
            max_rms: 0.0,
            sum_rms: 0.0,
        }
    }

    pub(crate) fn observe(&mut self, rms: f32) -> Option<(f32, f32, f32, u64)> {
        self.count += 1;
        self.min_rms = self.min_rms.min(rms);
        self.max_rms = self.max_rms.max(rms);
        self.sum_rms += rms as f64;
        if self.count < PCM_INGEST_RMS_LOG_INTERVAL_CHUNKS {
            return None;
        }
        let chunk_count = self.count;
        let summary = (
            self.min_rms,
            self.max_rms,
            (self.sum_rms / chunk_count as f64) as f32,
            chunk_count,
        );
        self.count = 0;
        self.min_rms = f32::INFINITY;
        self.max_rms = 0.0;
        self.sum_rms = 0.0;
        Some(summary)
    }
}

type StallProgressSlot = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
type InferencePercentSlot = Arc<Mutex<Option<Arc<dyn Fn(i32) + Send + Sync>>>>;
type WorkerFatalSlot = Arc<Mutex<Option<Arc<dyn Fn(TranscribeError) + Send + Sync>>>>;

struct StallProgressSlots {
    block: StallProgressSlot,
    inference: StallProgressSlot,
    engine_ready: StallProgressSlot,
    inference_attempted: StallProgressSlot,
    inference_pct: InferencePercentSlot,
    worker_fatal: WorkerFatalSlot,
}

fn stall_watchdog_clock() -> StallClock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    })
}

fn wire_stall_watchdog_inputs(
    lifecycle: &TranscribeLifecycleHook,
    block_progress: &StallProgressSlot,
    inference_progress: &StallProgressSlot,
) {
    let Some(watchdog) = lifecycle.stall_watchdog() else {
        return;
    };

    let block_watchdog = Arc::clone(&watchdog);
    *block_progress.lock().expect("lock block progress") =
        Some(Arc::new(move || block_watchdog.on_block_appended()));

    let inference_watchdog = Arc::clone(&watchdog);
    *inference_progress.lock().expect("lock inference progress") =
        Some(Arc::new(move || inference_watchdog.on_inference_success()));
}

struct CaptureFoundation {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pipeline: Arc<CapturePipelineState>,
    mic_gate: CaptureProcessingGate,
    ingest_level_cache:
        gijirec_presentation::tauri::capture_audio_controls::IngestLevelSnapshotCache,
    ingest_level_events: Arc<CachingIngestLevelEventEmitter>,
    ingest_level_emitter: Arc<IngestLevelEmitter>,
}

fn init_capture_foundation<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
) -> CaptureFoundation
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> =
        Arc::new(Mutex::new(DefaultCaptureOrchestrator::new(mic, system)));
    let pipeline = Arc::new(CapturePipelineState::new(streams));
    let mic_gate = pipeline.mic_gate().clone();
    let ingest_level_cache = Arc::new(Mutex::new(None));
    let ingest_level_events = Arc::new(CachingIngestLevelEventEmitter::new(Arc::clone(
        &ingest_level_cache,
    )));
    let ingest_level_emitter = Arc::new(IngestLevelEmitter::new(Arc::clone(&ingest_level_events)
        as Arc<dyn gijirec_presentation::transcribe::IngestLevelEventEmitter>));

    CaptureFoundation {
        orchestrator,
        pipeline,
        mic_gate,
        ingest_level_cache,
        ingest_level_events,
        ingest_level_emitter,
    }
}

fn init_device_selection(
    foundation: &CaptureFoundation,
) -> (
    Arc<dyn DeviceSelectionService>,
    Arc<LateBoundDeviceSelectionEvents>,
) {
    let device_selection_events = Arc::new(LateBoundDeviceSelectionEvents::new());
    let device_selection: Arc<dyn DeviceSelectionService> =
        Arc::new(DefaultDeviceSelectionService::new(
            DeviceSelectionStore::new(),
            DeviceEnumeratorPortAdapter::new(AudioDeviceEnumerator::new()),
            CaptureSelectionPortAdapter::new(
                Arc::clone(&foundation.orchestrator),
                Arc::clone(&foundation.pipeline),
            ),
            #[cfg(target_os = "macos")]
            MacosSpeakerPreflight { enabled: true },
            #[cfg(not(target_os = "macos"))]
            NoopSpeakerPreflight,
            DeviceSelectionEventsProxy(Arc::clone(&device_selection_events)),
            SystemClock,
            Arc::new(TracingDeviceSelectionObservability),
        ));
    (device_selection, device_selection_events)
}

struct PcmPipeline {
    pcm_ingest: Arc<PcmIngestConsumer>,
    rtrb_overflow_counter: Arc<std::sync::atomic::AtomicU64>,
    pcm_cons: rtrb::Consumer<f32>,
}

fn init_pcm_pipeline(
    foundation: &CaptureFoundation,
    ingest_level_emitter: &Arc<IngestLevelEmitter>,
) -> PcmPipeline {
    let (pcm_prod, pcm_cons) = rtrb::RingBuffer::<f32>::new(PCM_RTRB_CAPACITY_SAMPLES);
    let mut pcm_ingest = PcmIngestConsumer::new(pcm_prod);
    pcm_ingest.set_sequence_gap_callback(Arc::new(|from, to| {
        gijirec_presentation::transcribe::observability::log_pcm_sequence_gaps(from, to);
    }));
    let pcm_ingest_rms = Arc::new(Mutex::new(PcmIngestRmsAccumulator::new()));
    let ingest_level_rms = ingest_level_emitter.pcm_rms_callback();
    pcm_ingest.set_pcm_rms_callback(Arc::new({
        let accumulator = Arc::clone(&pcm_ingest_rms);
        move |rms| {
            let summary = accumulator
                .lock()
                .expect("lock pcm ingest rms")
                .observe(rms);
            if let Some((min_rms, max_rms, mean_rms, chunk_count)) = summary {
                gijirec_presentation::transcribe::observability::log_pcm_ingest_rms_summary(
                    min_rms,
                    max_rms,
                    mean_rms,
                    chunk_count,
                );
            }
            ingest_level_rms(rms);
        }
    }));
    let rtrb_overflow_counter = pcm_ingest.rtrb_overflow_counter();
    let pcm_ingest = Arc::new(pcm_ingest);
    foundation.pipeline.pcm_bus.register(Arc::clone(&pcm_ingest)
        as Arc<dyn gijirec_presentation::domain::audio::pcm_chunk::PcmChunkConsumer>);

    PcmPipeline {
        pcm_ingest,
        rtrb_overflow_counter,
        pcm_cons,
    }
}

struct AudioControlsBundle {
    service: Arc<dyn CaptureAudioControlsService>,
    events: Arc<LateBoundCaptureAudioControlsEvents>,
    hook: Arc<CaptureAudioControlsProcessingHook>,
}

fn init_audio_controls(foundation: &CaptureFoundation, pcm: &PcmPipeline) -> AudioControlsBundle {
    let capture_audio_controls_events = Arc::new(LateBoundCaptureAudioControlsEvents::new());
    let capture_audio_controls: Arc<dyn CaptureAudioControlsService> =
        Arc::new(DefaultCaptureAudioControlsService::new(
            CaptureAudioControlsStore::new(),
            OrchestratorCapturePhasePort {
                orchestrator: Arc::clone(&foundation.orchestrator),
            },
            CaptureAudioControlsApplyPortAdapter {
                mic_gate: foundation.mic_gate.clone(),
                pcm_ingest: Arc::clone(&pcm.pcm_ingest),
            },
            ComposeIngestSourcePort {
                orchestrator: Arc::clone(&foundation.orchestrator),
                pipeline: Arc::clone(&foundation.pipeline),
            },
            CaptureAudioControlsEventsProxy(Arc::clone(&capture_audio_controls_events)),
        ));
    let hook = Arc::new(CaptureAudioControlsProcessingHook::new(
        CaptureAudioControlsHookDeps {
            service: Arc::clone(&capture_audio_controls),
            mic_gate: foundation.mic_gate.clone(),
            pcm_ingest: Arc::clone(&pcm.pcm_ingest),
            ingest_level_emitter: Arc::clone(&foundation.ingest_level_emitter),
            ingest_level_cache: Arc::clone(&foundation.ingest_level_cache),
            ingest_source: ComposeIngestSourcePort {
                orchestrator: Arc::clone(&foundation.orchestrator),
                pipeline: Arc::clone(&foundation.pipeline),
            },
        },
    ));
    AudioControlsBundle {
        service: capture_audio_controls,
        events: capture_audio_controls_events,
        hook,
    }
}

fn init_block_bus(block_progress_slot: &StallProgressSlot) -> Arc<TranscriptBlockBus> {
    let block_bus = TranscriptBlockBus::new();
    block_bus.set_drop_callback(Arc::new(|drops| {
        gijirec_presentation::transcribe::observability::log_block_buffer_drop(drops);
    }));
    block_bus.set_publish_callback({
        let slot = Arc::clone(block_progress_slot);
        Arc::new(move || {
            if let Some(notify) = slot.lock().expect("lock block progress").as_ref() {
                notify();
            }
        })
    });
    Arc::new(block_bus)
}

fn wire_worker_observability_callbacks(
    worker: &mut TranscribeWorker,
    model_orchestrator: &SharedModelOrchestrator,
    inference_progress_slot: &StallProgressSlot,
) {
    let model_orchestrator_for_cycles = Arc::clone(model_orchestrator);
    worker.set_batch_cycle_started_callback(Arc::new(move |event| {
        gijirec_presentation::transcribe::observability::log_batch_cycle_started(
            event.cycle_id,
            event.samples_count,
            event.pcm_backlog_seconds,
            event.rtrb_overflow_count,
        );
        match model_orchestrator_for_cycles
            .lock()
            .expect("lock model orchestrator")
            .try_apply_pending_variant()
        {
            Ok(Some(ApplyVariantOutcome::Applied { path })) => {
                if let Some(variant) = model_orchestrator_for_cycles
                    .lock()
                    .expect("lock model orchestrator")
                    .active_variant()
                {
                    gijirec_presentation::transcribe::observability::log_model_variant_applied(
                        variant,
                    );
                }
                Some(path)
            }
            Ok(_) | Err(_) => None,
        }
    }));
    worker.set_batch_cycle_completed_callback(Arc::new(|event| {
        gijirec_presentation::transcribe::observability::log_batch_cycle_completed(
            event.cycle_id,
            event.duration_ms,
            event.samples_count,
            event.segments_count,
        );
    }));
    worker.set_inference_window_level_callback(Arc::new(|level| {
        gijirec_presentation::transcribe::observability::log_inference_window_level(
            level.window_rms,
            level.samples_count,
            level.inference_skipped,
        );
    }));
    worker.set_inference_latency_callback({
        let slot = Arc::clone(inference_progress_slot);
        Arc::new(move |ms| {
            gijirec_presentation::transcribe::observability::log_inference_latency(ms);
            if let Some(notify) = slot.lock().expect("lock inference progress").as_ref() {
                notify();
            }
        })
    });
}

fn wire_worker_stall_callbacks(worker: &mut TranscribeWorker, slots: &StallProgressSlots) {
    worker.set_engine_ready_callback({
        let slot = Arc::clone(&slots.engine_ready);
        Arc::new(move || {
            if let Some(notify) = slot.lock().expect("lock engine ready").as_ref() {
                notify();
            }
        })
    });
    worker.set_inference_attempted_callback({
        let slot = Arc::clone(&slots.inference_attempted);
        Arc::new(move || {
            if let Some(notify) = slot.lock().expect("lock inference attempted").as_ref() {
                notify();
            }
        })
    });
    worker.set_inference_progress_callback({
        let slot = Arc::clone(&slots.inference_pct);
        Arc::new(move |percent| {
            if percent == 0 || percent == 100 || percent % 10 == 0 {
                gijirec_presentation::transcribe::observability::log_inference_progress(percent);
            }
            if let Some(notify) = slot.lock().expect("lock inference progress pct").as_ref() {
                notify(percent);
            }
        })
    });
    worker.set_fatal_error_callback({
        let slot = Arc::clone(&slots.worker_fatal);
        Arc::new(move |err| {
            if let Some(notify) = slot.lock().expect("lock worker fatal").as_ref() {
                notify(err);
            }
        })
    });
}

struct TranscribeWorkerWiring<'a, C> {
    block_emitter: Arc<BlockEmitter<C>>,
    pcm_cons: rtrb::Consumer<f32>,
    rtrb_overflow_counter: Arc<std::sync::atomic::AtomicU64>,
    model_orchestrator: &'a SharedModelOrchestrator,
    slots: &'a StallProgressSlots,
}

fn init_transcribe_worker<C: TranscriptBlockConsumer + 'static>(
    wiring: TranscribeWorkerWiring<'_, C>,
) -> TranscribeWorkerPortAdapter {
    let mut worker =
        TranscribeWorker::new(Arc::clone(&wiring.block_emitter) as Arc<dyn TranscriptSegmentSink>);
    worker.attach_pcm_consumer(wiring.pcm_cons);
    worker.set_rtrb_overflow_counter(wiring.rtrb_overflow_counter);
    wire_worker_observability_callbacks(
        &mut worker,
        wiring.model_orchestrator,
        &wiring.slots.inference,
    );
    wire_worker_stall_callbacks(&mut worker, wiring.slots);
    TranscribeWorkerPortAdapter::from_worker(worker)
}

struct MockEmitter;

impl TranscribeEventEmitter for MockEmitter {
    fn emit_phase_changed(
        &self,
        _phase: gijirec_presentation::domain::transcribe::TranscribePhase,
    ) -> Result<(), gijirec_presentation::transcribe::TranscribeEmitError> {
        Ok(())
    }
    fn emit_model_progress(
        &self,
        _progress: &gijirec_presentation::application::transcribe::ModelDownloadProgress,
    ) -> Result<(), gijirec_presentation::transcribe::TranscribeEmitError> {
        Ok(())
    }
    fn emit_error(
        &self,
        _error: &gijirec_presentation::domain::transcribe::TranscribeError,
    ) -> Result<(), gijirec_presentation::transcribe::TranscribeEmitError> {
        Ok(())
    }
}

fn init_transcribe_lifecycle(
    transcribe_orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    slots: &StallProgressSlots,
) -> Arc<TranscribeLifecycleHook> {
    let dummy_emitter = Arc::new(MockEmitter);
    let transcribe_lifecycle = Arc::new(TranscribeLifecycleHook::with_stall_watchdog(
        transcribe_orchestrator,
        dummy_emitter,
        stall_watchdog_clock(),
    ));
    wire_stall_watchdog_inputs(
        transcribe_lifecycle.as_ref(),
        &slots.block,
        &slots.inference,
    );
    if let Some(watchdog) = transcribe_lifecycle.stall_watchdog() {
        *slots.engine_ready.lock().expect("lock engine ready slot") =
            Some(Arc::new(move || watchdog.on_engine_ready()));
    }
    if let Some(watchdog) = transcribe_lifecycle.stall_watchdog() {
        *slots
            .inference_attempted
            .lock()
            .expect("lock inference attempted slot") =
            Some(Arc::new(move || watchdog.on_inference_attempted()));
    }
    if let Some(watchdog) = transcribe_lifecycle.stall_watchdog() {
        *slots
            .inference_pct
            .lock()
            .expect("lock inference progress slot") =
            Some(Arc::new(move |_percent| watchdog.on_inference_progress()));
    }
    {
        let lifecycle = Arc::clone(&transcribe_lifecycle);
        *slots.worker_fatal.lock().expect("lock worker fatal slot") = Some(Arc::new(move |err| {
            lifecycle.on_worker_engine_failed(err);
        }));
    }
    transcribe_lifecycle
}

pub(crate) fn compose_with_ports_and_model_orchestrator<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
    model_orchestrator: SharedModelOrchestrator,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    let foundation = init_capture_foundation(mic, system, streams);
    let (device_selection, device_selection_events) = init_device_selection(&foundation);

    let slots = StallProgressSlots {
        block: Arc::new(Mutex::new(None)),
        inference: Arc::new(Mutex::new(None)),
        engine_ready: Arc::new(Mutex::new(None)),
        inference_attempted: Arc::new(Mutex::new(None)),
        inference_pct: Arc::new(Mutex::new(None)),
        worker_fatal: Arc::new(Mutex::new(None)),
    };

    let pcm = init_pcm_pipeline(&foundation, &foundation.ingest_level_emitter);
    let audio_controls = init_audio_controls(&foundation, &pcm);
    let block_bus = init_block_bus(&slots.block);
    let block_emitter = Arc::new(BlockEmitter::new(Arc::clone(&block_bus)));

    let worker_adapter = init_transcribe_worker(TranscribeWorkerWiring {
        block_emitter: Arc::clone(&block_emitter),
        pcm_cons: pcm.pcm_cons,
        rtrb_overflow_counter: pcm.rtrb_overflow_counter,
        model_orchestrator: &model_orchestrator,
        slots: &slots,
    });
    let context_adapter = WhisperContextPortAdapter::new();
    let transcribe_orchestrator = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
        worker_adapter,
        context_adapter,
        Arc::clone(&model_orchestrator),
        DEFAULT_TRANSCRIBE_STOP_TIMEOUT,
    )));
    let transcribe_lifecycle = init_transcribe_lifecycle(
        Arc::clone(&transcribe_orchestrator) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        &slots,
    );

    ComposedCapture {
        orchestrator: foundation.orchestrator,
        device_selection,
        device_selection_events,
        pipeline: foundation.pipeline,
        capture_audio_controls: audio_controls.service,
        capture_audio_controls_events: audio_controls.events,
        capture_audio_controls_hook: audio_controls.hook,
        ingest_level_events: foundation.ingest_level_events,
        ingest_level_cache: foundation.ingest_level_cache,
        transcribe_lifecycle,
        transcribe_bus: block_bus,
        transcribe_orchestrator,
        model_orchestrator,
    }
}
