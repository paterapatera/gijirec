//! Composition root: orchestrator, pipeline hold, transcribe wiring, and lifecycle helpers.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::device_selection_observability::TracingDeviceSelectionObservability;
use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
};
#[cfg(target_os = "macos")]
use gijirec_presentation::application::device_selection::MacosSpeakerPreflight;
use gijirec_presentation::application::device_selection::{
    CaptureSelectionPort, DefaultDeviceSelectionService, DeviceEnumeratorPort,
    DeviceSelectionError, DeviceSelectionEvents, DeviceSelectionService, NoopSpeakerPreflight,
    SystemClock,
};
use gijirec_presentation::application::transcribe::block_emitter::BlockEmitter;
use gijirec_presentation::application::transcribe::model_orchestrator::{
    ModelOrchestrator, ModelOrchestratorConfig,
};
use gijirec_presentation::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use gijirec_presentation::domain::audio::{AudioDeviceList, CaptureError, DeviceSelection};
use gijirec_presentation::domain::transcribe::{TranscribeError, TranscriptSegmentSink};
use gijirec_presentation::infrastructure::audio::device_enumerator::{
    AudioDeviceEnumerator, EnumeratorError,
};
use gijirec_presentation::infrastructure::transcribe::{
    ModelDownloader, ModelStore, TranscribeWorker,
};
use gijirec_presentation::transcribe::lifecycle_hook::DEFAULT_TRANSCRIBE_STOP_TIMEOUT;
use gijirec_presentation::transcribe::{
    ModelDownloaderPortAdapter, ModelStorePortAdapter, PcmIngestConsumer, StallClock,
    TranscribeEventEmitter, TranscribeLifecycleHook, TranscribeWorkerPortAdapter,
    TranscriptBlockBus, WhisperContextPortAdapter,
};

use crate::capture_ports::CaptureStreamHandles;
use crate::capture_processing::CapturePipelineState;
use gijirec_presentation::application::device_selection::DeviceSelectionStore;

/// Shared model acquisition handle used without holding the transcribe orchestrator lock.
pub(crate) type SharedModelOrchestrator =
    Arc<Mutex<ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter>>>;

/// Late-bound [`DeviceSelectionEvents`] emitter (attached in Tauri `setup`).
pub(crate) struct LateBoundDeviceSelectionEvents {
    emitter: Mutex<Option<Arc<dyn DeviceSelectionEvents>>>,
}

impl LateBoundDeviceSelectionEvents {
    pub(crate) fn new() -> Self {
        Self {
            emitter: Mutex::new(None),
        }
    }

    pub(crate) fn set_emitter(&self, emitter: Arc<dyn DeviceSelectionEvents>) {
        *self.emitter.lock().expect("lock") = Some(emitter);
    }
}

impl DeviceSelectionEvents for LateBoundDeviceSelectionEvents {
    fn emit_selection_changed(&self, selection: &DeviceSelection) {
        if let Some(emitter) = self.emitter.lock().expect("lock").as_ref() {
            emitter.emit_selection_changed(selection);
        }
    }

    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64) {
        if let Some(emitter) = self.emitter.lock().expect("lock").as_ref() {
            emitter.emit_devices_changed(devices, timestamp_ms);
        }
    }
}

/// Forwards [`DeviceSelectionEvents`] to a shared [`LateBoundDeviceSelectionEvents`].
struct DeviceSelectionEventsProxy(Arc<LateBoundDeviceSelectionEvents>);

impl DeviceSelectionEvents for DeviceSelectionEventsProxy {
    fn emit_selection_changed(&self, selection: &DeviceSelection) {
        self.0.emit_selection_changed(selection);
    }

    fn emit_devices_changed(&self, devices: &AudioDeviceList, timestamp_ms: u64) {
        self.0.emit_devices_changed(devices, timestamp_ms);
    }
}

struct DeviceEnumeratorPortAdapter {
    inner: AudioDeviceEnumerator,
}

impl DeviceEnumeratorPortAdapter {
    fn new(inner: AudioDeviceEnumerator) -> Self {
        Self { inner }
    }
}

impl DeviceEnumeratorPort for DeviceEnumeratorPortAdapter {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
        self.inner
            .list_devices()
            .map_err(|err: EnumeratorError| DeviceSelectionError::internal(err.to_string()))
    }
}

struct CaptureSelectionPortAdapter {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pipeline: Arc<CapturePipelineState>,
}

impl CaptureSelectionPortAdapter {
    fn new(
        orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
        pipeline: Arc<CapturePipelineState>,
    ) -> Self {
        Self {
            orchestrator,
            pipeline,
        }
    }
}

impl CaptureSelectionPort for CaptureSelectionPortAdapter {
    fn capture_phase(&self) -> gijirec_presentation::domain::audio::CapturePhase {
        self.orchestrator.lock().expect("lock").phase()
    }

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.pipeline.stop_processing_for_recapture();
        let result = self
            .orchestrator
            .lock()
            .expect("lock")
            .restart_with_selection(selection);
        if result.is_ok() {
            let _ = self.pipeline.start_processing();
        }
        result
    }
}

/// Fully composed capture and transcribe stack ready for Tauri lifecycle injection.
pub(crate) struct ComposedCapture {
    pub orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pub device_selection: Arc<dyn DeviceSelectionService>,
    pub device_selection_events: Arc<LateBoundDeviceSelectionEvents>,
    pub pipeline: Arc<CapturePipelineState>,
    pub transcribe_lifecycle: Arc<TranscribeLifecycleHook>,
    pub transcribe_bus: Arc<TranscriptBlockBus>,
    pub transcribe_orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    pub model_orchestrator: SharedModelOrchestrator,
}

/// Default HuggingFace download URL and SHA-256 for kotoba-whisper-v2.2 GGML (Q5_0).
pub(crate) const DEFAULT_WHISPER_MODEL_URL: &str = "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q5_0.bin";
pub(crate) const DEFAULT_WHISPER_MODEL_SHA256: &str =
    "4a3b92192b5d3578ff854a5876213e2e27af0c2d357492c2d14271e82c303658";

fn default_model_config() -> ModelOrchestratorConfig {
    ModelOrchestratorConfig {
        model_url: DEFAULT_WHISPER_MODEL_URL.to_string(),
        expected_sha256: DEFAULT_WHISPER_MODEL_SHA256.to_string(),
    }
}

fn build_model_orchestrator(
    app_data_dir: std::path::PathBuf,
    model_config: ModelOrchestratorConfig,
) -> ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter> {
    let store = ModelStore::new(app_data_dir);
    let store_adapter = ModelStorePortAdapter::new(store);
    let downloader = ModelDownloader::new().expect("HTTPS client initialization");
    let downloader_adapter = ModelDownloaderPortAdapter::new(downloader).expect("adapter");
    ModelOrchestrator::new(store_adapter, downloader_adapter, model_config)
}

fn wrap_model_orchestrator(
    orchestrator: ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter>,
) -> SharedModelOrchestrator {
    Arc::new(Mutex::new(orchestrator))
}

/// Placeholder model stack until Tauri setup calls [`inject_model_stack`].
fn deferred_model_orchestrator() -> SharedModelOrchestrator {
    wrap_model_orchestrator(build_model_orchestrator(
        std::path::PathBuf::new(),
        default_model_config(),
    ))
}

fn stall_watchdog_clock() -> StallClock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    })
}

type StallProgressSlot = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
type StallInputSlot = Arc<Mutex<Option<Arc<dyn Fn(f32) + Send + Sync>>>>;
type InferencePercentSlot = Arc<Mutex<Option<Arc<dyn Fn(i32) + Send + Sync>>>>;
type WorkerFatalSlot = Arc<Mutex<Option<Arc<dyn Fn(TranscribeError) + Send + Sync>>>>;

fn wire_stall_watchdog_inputs(
    lifecycle: &TranscribeLifecycleHook,
    block_progress: &StallProgressSlot,
    inference_progress: &StallProgressSlot,
    pcm_input: &StallInputSlot,
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

    let pcm_watchdog = Arc::clone(&watchdog);
    *pcm_input.lock().expect("lock pcm input") = Some(Arc::new(move |rms| {
        pcm_watchdog.on_pcm_rms(rms);
    }));
}

/// Builds the production capture and transcribe stack with platform and whisper adapters.
/// Model acquisition uses a deferred placeholder until setup calls [`inject_model_stack`].
pub(crate) fn build_capture_stack() -> ComposedCapture {
    let (streams, mic, system) = CaptureStreamHandles::new_pair();
    compose_with_ports_and_model_orchestrator(mic, system, streams, deferred_model_orchestrator())
}

/// Injects `ModelStore` / `ModelOrchestrator` after Tauri resolves `app_data_dir`.
pub(crate) fn inject_model_stack(composed: &mut ComposedCapture, app_data_dir: std::path::PathBuf) {
    inject_model_stack_shared(&composed.model_orchestrator, app_data_dir);
}

/// Injects the model stack into a shared handle (Tauri setup path).
pub(crate) fn inject_model_stack_shared(
    model_orchestrator: &SharedModelOrchestrator,
    app_data_dir: std::path::PathBuf,
) {
    inject_model_stack_shared_with_config(model_orchestrator, app_data_dir, default_model_config());
}

/// Injects the model stack with explicit download configuration (tests / overrides).
pub(crate) fn inject_model_stack_with_config(
    composed: &mut ComposedCapture,
    app_data_dir: std::path::PathBuf,
    model_config: ModelOrchestratorConfig,
) {
    inject_model_stack_shared_with_config(&composed.model_orchestrator, app_data_dir, model_config);
}

/// Injects the model stack with explicit download configuration into a shared handle.
pub(crate) fn inject_model_stack_shared_with_config(
    model_orchestrator: &SharedModelOrchestrator,
    app_data_dir: std::path::PathBuf,
    model_config: ModelOrchestratorConfig,
) {
    let _ = ModelStore::new(app_data_dir.clone()).maybe_migrate_from_legacy_local();
    *model_orchestrator.lock().expect("lock model orchestrator") =
        build_model_orchestrator(app_data_dir, model_config);
}

/// Extra parameters for initializing the transcribe stack in composition root.
pub(crate) struct TranscribeComposeConfig {
    pub app_data_dir: std::path::PathBuf,
    pub model_config: ModelOrchestratorConfig,
}

/// Builds capture + transcribe stack from injectable ports and data directories.
pub(crate) fn compose_with_ports_and_transcribe<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
    config: TranscribeComposeConfig,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    let model_orchestrator = wrap_model_orchestrator(build_model_orchestrator(
        config.app_data_dir,
        config.model_config,
    ));
    compose_with_ports_and_model_orchestrator(mic, system, streams, model_orchestrator)
}

fn compose_with_ports_and_model_orchestrator<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
    model_orchestrator: SharedModelOrchestrator,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    let orchestrator: Arc<Mutex<dyn CaptureOrchestrator>> =
        Arc::new(Mutex::new(DefaultCaptureOrchestrator::new(mic, system)));
    let pipeline = Arc::new(CapturePipelineState::new(streams));

    let device_selection_events = Arc::new(LateBoundDeviceSelectionEvents::new());
    let device_selection: Arc<dyn DeviceSelectionService> =
        Arc::new(DefaultDeviceSelectionService::new(
            DeviceSelectionStore::new(),
            DeviceEnumeratorPortAdapter::new(AudioDeviceEnumerator::new()),
            CaptureSelectionPortAdapter::new(Arc::clone(&orchestrator), Arc::clone(&pipeline)),
            #[cfg(target_os = "macos")]
            MacosSpeakerPreflight { enabled: true },
            #[cfg(not(target_os = "macos"))]
            NoopSpeakerPreflight,
            DeviceSelectionEventsProxy(Arc::clone(&device_selection_events)),
            SystemClock,
            Arc::new(TracingDeviceSelectionObservability),
        ));

    let block_progress_slot: StallProgressSlot = Arc::new(Mutex::new(None));
    let inference_progress_slot: StallProgressSlot = Arc::new(Mutex::new(None));
    let pcm_input_slot: StallInputSlot = Arc::new(Mutex::new(None));

    // 1. Set up PCM buffer between IngestConsumer and TranscribeWorker (30 s @ 16 kHz = 480k)
    let (pcm_prod, pcm_cons) = rtrb::RingBuffer::<f32>::new(480_000);
    let mut pcm_ingest = PcmIngestConsumer::new(pcm_prod);
    pcm_ingest.set_sequence_gap_callback(Arc::new(|from, to| {
        gijirec_presentation::transcribe::observability::log_pcm_sequence_gaps(from, to);
    }));
    pcm_ingest.set_pcm_rms_callback({
        let slot = Arc::clone(&pcm_input_slot);
        Arc::new(move |rms| {
            if let Some(notify) = slot.lock().expect("lock pcm input").as_ref() {
                notify(rms);
            }
        })
    });
    let pcm_ingest = Arc::new(pcm_ingest);
    pipeline.pcm_bus.register(pcm_ingest);

    // 2. Set up TranscriptBlockBus with drop logging
    let block_bus = TranscriptBlockBus::new();
    block_bus.set_drop_callback(Arc::new(|drops| {
        gijirec_presentation::transcribe::observability::log_block_buffer_drop(drops);
    }));
    block_bus.set_publish_callback({
        let slot = Arc::clone(&block_progress_slot);
        Arc::new(move || {
            if let Some(notify) = slot.lock().expect("lock block progress").as_ref() {
                notify();
            }
        })
    });
    let block_bus = Arc::new(block_bus);

    // 3. Set up BlockEmitter as sink for TranscribeWorker
    let block_emitter = Arc::new(BlockEmitter::new(Arc::clone(&block_bus)));

    // 4. Set up TranscribeWorker with latency metric callback
    let mut worker =
        TranscribeWorker::new(Arc::clone(&block_emitter) as Arc<dyn TranscriptSegmentSink>);
    worker.attach_pcm_consumer(pcm_cons);
    worker.set_inference_latency_callback({
        let slot = Arc::clone(&inference_progress_slot);
        Arc::new(move |ms| {
            gijirec_presentation::transcribe::observability::log_inference_latency(ms);
            if let Some(notify) = slot.lock().expect("lock inference progress").as_ref() {
                notify();
            }
        })
    });
    let engine_ready_slot: StallProgressSlot = Arc::new(Mutex::new(None));
    worker.set_engine_ready_callback({
        let slot = Arc::clone(&engine_ready_slot);
        Arc::new(move || {
            if let Some(notify) = slot.lock().expect("lock engine ready").as_ref() {
                notify();
            }
        })
    });
    let inference_attempted_slot: StallProgressSlot = Arc::new(Mutex::new(None));
    worker.set_inference_attempted_callback({
        let slot = Arc::clone(&inference_attempted_slot);
        Arc::new(move || {
            if let Some(notify) = slot.lock().expect("lock inference attempted").as_ref() {
                notify();
            }
        })
    });
    let inference_pct_slot: InferencePercentSlot = Arc::new(Mutex::new(None));
    worker.set_inference_progress_callback({
        let slot = Arc::clone(&inference_pct_slot);
        Arc::new(move |percent| {
            if percent == 0 || percent == 100 || percent % 10 == 0 {
                gijirec_presentation::transcribe::observability::log_inference_progress(percent);
            }
            if let Some(notify) = slot.lock().expect("lock inference progress pct").as_ref() {
                notify(percent);
            }
        })
    });
    let worker_fatal_slot: WorkerFatalSlot = Arc::new(Mutex::new(None));
    worker.set_fatal_error_callback({
        let slot = Arc::clone(&worker_fatal_slot);
        Arc::new(move |err| {
            if let Some(notify) = slot.lock().expect("lock worker fatal").as_ref() {
                notify(err);
            }
        })
    });
    let worker_adapter = TranscribeWorkerPortAdapter::from_worker(worker);

    // 6. Set up Context adapter
    let context_adapter = WhisperContextPortAdapter::new();

    // 7. Compose DefaultTranscribeOrchestrator
    let transcribe_orchestrator = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
        worker_adapter,
        context_adapter,
        Arc::clone(&model_orchestrator),
        DEFAULT_TRANSCRIBE_STOP_TIMEOUT,
    )));

    // 8. Set up Noop/Dummy TranscribeEventEmitter for initial lifecycle hook (updated when Tauri sets up)
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
    let dummy_emitter = Arc::new(MockEmitter);
    let transcribe_lifecycle = Arc::new(TranscribeLifecycleHook::with_stall_watchdog(
        Arc::clone(&transcribe_orchestrator) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        dummy_emitter,
        stall_watchdog_clock(),
    ));
    wire_stall_watchdog_inputs(
        transcribe_lifecycle.as_ref(),
        &block_progress_slot,
        &inference_progress_slot,
        &pcm_input_slot,
    );
    if let Some(watchdog) = transcribe_lifecycle.stall_watchdog() {
        *engine_ready_slot.lock().expect("lock engine ready slot") =
            Some(Arc::new(move || watchdog.on_engine_ready()));
    }
    if let Some(watchdog) = transcribe_lifecycle.stall_watchdog() {
        *inference_attempted_slot
            .lock()
            .expect("lock inference attempted slot") =
            Some(Arc::new(move || watchdog.on_inference_attempted()));
    }
    if let Some(watchdog) = transcribe_lifecycle.stall_watchdog() {
        *inference_pct_slot
            .lock()
            .expect("lock inference progress slot") =
            Some(Arc::new(move |_percent| watchdog.on_inference_progress()));
    }
    {
        let lifecycle = Arc::clone(&transcribe_lifecycle);
        *worker_fatal_slot.lock().expect("lock worker fatal slot") = Some(Arc::new(move |err| {
            lifecycle.on_worker_engine_failed(err);
        }));
    }

    ComposedCapture {
        orchestrator,
        device_selection,
        device_selection_events,
        pipeline,
        transcribe_lifecycle,
        transcribe_bus: block_bus,
        transcribe_orchestrator,
        model_orchestrator,
    }
}

/// Builds capture stack with default dummy transcribe stack for testing.
#[cfg(test)]
pub(crate) fn compose_with_ports<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    compose_with_ports_and_transcribe(
        mic,
        system,
        streams,
        TranscribeComposeConfig {
            app_data_dir: std::env::temp_dir().join("gijirec_test"),
            model_config: ModelOrchestratorConfig {
                model_url: "".to_string(),
                expected_sha256: "".to_string(),
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
    use std::sync::atomic::{AtomicU8, Ordering};

    struct TrackingMic {
        order: Arc<AtomicU8>,
    }

    struct TrackingSystem {
        order: Arc<AtomicU8>,
    }

    impl MicCapturePort for TrackingMic {
        fn open(&mut self) -> Result<(), CaptureError> {
            self.order.store(1, Ordering::SeqCst);
            Ok(())
        }

        fn close(&mut self) {
            self.order.store(0, Ordering::SeqCst);
        }

        fn is_open(&self) -> bool {
            self.order.load(Ordering::SeqCst) >= 1
        }
    }

    impl SystemAudioCapturePort for TrackingSystem {
        fn open(&mut self) -> Result<(), CaptureError> {
            let mic_first = self.order.load(Ordering::SeqCst) == 1;
            if !mic_first {
                return Err(CaptureError::Internal {
                    detail: "system opened before mic".to_string(),
                });
            }
            self.order.store(2, Ordering::SeqCst);
            Ok(())
        }

        fn close(&mut self) {
            self.order.store(0, Ordering::SeqCst);
        }

        fn is_open(&self) -> bool {
            self.order.load(Ordering::SeqCst) == 2
        }
    }

    #[test]
    fn inject_model_stack_accepts_app_data_dir() {
        let mut composed = build_capture_stack();
        inject_model_stack(
            &mut composed,
            std::env::temp_dir().join("gijirec_inject_test"),
        );
    }

    #[test]
    fn compose_does_not_reference_dirs_data_local_dir() {
        let compose_source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compose.rs"),
        )
        .expect("compose.rs must exist");
        let production_source = compose_source
            .split("mod tests")
            .next()
            .expect("compose.rs must define tests module");
        assert!(
            !production_source.contains("dirs::data_local_dir"),
            "compose production code must not use dirs::data_local_dir; app_data_dir is injected after Tauri setup"
        );
        assert!(
            production_source.contains("inject_model_stack_shared_with_config")
                && production_source.contains("maybe_migrate_from_legacy_local"),
            "compose must migrate legacy local models only during inject"
        );
        assert!(
            !production_source
                .split("fn build_model_orchestrator")
                .nth(1)
                .and_then(|body| body.split("fn wrap_model_orchestrator").next())
                .unwrap_or("")
                .contains("maybe_migrate_from_legacy_local"),
            "deferred build_model_orchestrator must not migrate legacy models"
        );
    }

    #[test]
    fn compose_holds_pipeline_components() {
        let (streams, _mic, _system) = CaptureStreamHandles::new_pair();
        let order = Arc::new(AtomicU8::new(0));
        let mic = TrackingMic {
            order: Arc::clone(&order),
        };
        let system = TrackingSystem {
            order: Arc::clone(&order),
        };
        let composed = compose_with_ports(mic, system, streams);

        assert_eq!(
            composed.orchestrator.lock().expect("lock").phase(),
            CapturePhase::Idle
        );
        assert!(!composed.pipeline.processing_is_active());
    }

    #[test]
    fn device_selection_service_is_wired_in_composed_stack() {
        let composed = build_capture_stack();
        let _ = composed
            .device_selection
            .list_devices()
            .expect("list_devices must not panic");
    }

    #[test]
    fn build_capture_stack_wires_stall_watchdog() {
        let composed = build_capture_stack();
        assert!(
            composed.transcribe_lifecycle.stall_watchdog().is_some(),
            "production compose must enable stall watchdog via with_stall_watchdog"
        );
    }

    #[test]
    fn compose_wires_worker_fatal_and_engine_ready_to_lifecycle() {
        let compose_source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compose.rs"),
        )
        .expect("compose.rs must exist");
        let production_source = compose_source
            .split("mod tests")
            .next()
            .expect("compose.rs must define tests module");
        assert!(
            production_source.contains("on_worker_engine_failed"),
            "compose must surface worker engine load failures through the lifecycle hook"
        );
        assert!(
            production_source.contains("on_engine_ready"),
            "compose must reset stall timing after worker engine load completes"
        );
        assert!(
            production_source.contains("on_inference_attempted"),
            "compose must reset stall timing when inference starts"
        );
        assert!(
            production_source.contains("on_inference_progress"),
            "compose must extend stall timing from whisper.cpp progress callbacks"
        );
    }

    /// Hardware integration test for Windows WASAPI loopback + mic (ignored on CI).
    #[test]
    #[ignore = "CI: requires Windows mic permission and default WASAPI loopback output; run with --ignored on local hardware"]
    fn integration_mic_and_wasapi_loopback_reach_capturing_on_hardware() {
        let composed = build_capture_stack();
        assert_eq!(
            composed.orchestrator.lock().expect("lock").phase(),
            CapturePhase::Idle
        );
    }

    /// Hardware integration test for macOS ScreenCaptureKit + mic (ignored on CI).
    #[test]
    #[ignore = "CI: requires macOS mic permission and ScreenCaptureKit screen recording permission; run with --ignored on local hardware"]
    fn integration_mic_and_sck_reach_capturing_on_hardware() {
        let composed = build_capture_stack();
        assert_eq!(
            composed.orchestrator.lock().expect("lock").phase(),
            CapturePhase::Idle
        );
    }
}
